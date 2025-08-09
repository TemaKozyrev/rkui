use std::collections::{HashMap, VecDeque, BinaryHeap};
use std::cmp::Reverse;
use std::time::Duration;

use rdkafka::message::Message as RdMessage;

use crate::kafka::{Kafka, UiMessage};

/// Strategy: simple sequential consumption for a single partition.
pub fn consume_sequential(kafka: &Kafka, ends: &HashMap<i32, i64>, parts: &Vec<i32>, limit: usize) -> anyhow::Result<Vec<UiMessage>> {
    let mut out: Vec<UiMessage> = Vec::with_capacity(limit);
    let mut idle_loops = 0;
    while out.len() < limit && idle_loops < 20 {
        match kafka.consumer.as_ref().poll(Duration::from_millis(200)) {
            Some(Ok(m)) => {
                let partition = m.partition();
                let offset = m.offset();
                let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                if offset >= end {
                    let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                    done.insert(partition);
                    if parts.iter().all(|p| done.contains(p)) { break; }
                    continue;
                }
                let (key, payload, decoding_error) = kafka.decode(m.key(), m.payload());
                let timestamp = match m.timestamp() {
                    rdkafka::message::Timestamp::NotAvailable => String::from(""),
                    rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                        if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { dt.to_rfc3339() } else { String::new() }
                    }
                };
                out.push(UiMessage { id: format!("{}-{}", partition, offset), partition, key, offset, message: payload, timestamp, decoding_error });
                if offset >= end - 1 {
                    let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                    done.insert(partition);
                    if parts.iter().all(|p| done.contains(p)) { break; }
                }
            }
            Some(Err(_)) | None => { idle_loops += 1; }
        }
    }
    Ok(out)
}

/// Strategy: merge messages from multiple partitions by timestamp using per-partition buffers.
pub fn consume_merge(kafka: &Kafka, ends: &HashMap<i32, i64>, parts: &Vec<i32>, limit: usize) -> anyhow::Result<Vec<UiMessage>> {
    let mut out: Vec<UiMessage> = Vec::with_capacity(limit);

    // Step 1: ensure each active partition has at least one buffered message
    let mut idle_loops = 0;
    loop {
        // Determine active (not-done) partitions without cloning entire set
        let active_parts: Vec<i32> = {
            let done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
            parts.iter().copied().filter(|p| !done.contains(p)).collect()
        };
        let mut bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
        let mut need = false;
        for p in active_parts {
            let e = bufs.entry(p).or_insert_with(VecDeque::new);
            if e.front().is_none() { need = true; }
        }
        drop(bufs);
        if !need { break; }
        if idle_loops >= 20 { break; }
        match kafka.consumer.as_ref().poll(Duration::from_millis(200)) {
            Some(Ok(m)) => {
                let partition = m.partition();
                let offset = m.offset();
                let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                if offset >= end {
                    let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                    done.insert(partition);
                    continue;
                }
                let (key, payload, decoding_error) = kafka.decode(m.key(), m.payload());
                let (ts_ms, ts_str) = match m.timestamp() {
                    rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                    rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                        if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                    }
                };
                let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key, offset, message: payload, timestamp: ts_str, decoding_error };
                let mut bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                bufs.entry(partition).or_insert_with(VecDeque::new).push_back((ts_ms, ui));
                if offset >= end - 1 {
                    let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                    done.insert(partition);
                }
            }
            Some(Err(_)) | None => { idle_loops += 1; }
        }
    }

    // Step 2: merge-pop by minimal timestamp using a min-heap
    let mut heap: BinaryHeap<(Reverse<(i64, i32, i64)>, i32)> = BinaryHeap::new();
    {
        let bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
        for (&p, q) in bufs.iter() {
            if let Some((ts, ui)) = q.front() {
                let key = (*ts, p, ui.offset);
                heap.push((Reverse(key), p));
            }
        }
    }

    while out.len() < limit {
        if heap.is_empty() {
            if idle_loops >= 20 { break; }
            // try to poll for more data and rebuild heap
            match kafka.consumer.as_ref().poll(Duration::from_millis(200)) {
                Some(Ok(m)) => {
                    let partition = m.partition();
                    let offset = m.offset();
                    let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                    if offset >= end {
                        let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                        done.insert(partition);
                    } else {
                        let (key_s, payload, decoding_error) = kafka.decode(m.key(), m.payload());
                        let (ts_ms, ts_str) = match m.timestamp() {
                            rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                            rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                                if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                            }
                        };
                        let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key: key_s, offset, message: payload, timestamp: ts_str, decoding_error };
                        let mut bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                        let q = bufs.entry(partition).or_insert_with(VecDeque::new);
                        let was_empty = q.is_empty();
                        q.push_back((ts_ms, ui));
                        drop(bufs);
                        if was_empty {
                            heap.push((Reverse((ts_ms, partition, offset)), partition));
                        }
                    }
                }
                Some(Err(_)) | None => { idle_loops += 1; }
            }
            continue;
        }

        // Pop the smallest timestamp item
        let Some((_key, pick_p)) = heap.pop() else { break; };
        let maybe_item = {
            let mut bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
            if let Some(q) = bufs.get_mut(&pick_p) { q.pop_front() } else { None }
        };
        let Some((ts_emitted, ui)) = maybe_item else {
            continue;
        };
        let _ = ts_emitted;
        let end_for_p = ends.get(&pick_p).cloned().unwrap_or(i64::MAX);
        if ui.offset >= end_for_p - 1 {
            let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
            done.insert(pick_p);
        }
        out.push(ui);

        // Push next head or try to refill this partition
        let mut need_refill = false;
        {
            let bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
            if let Some(q) = bufs.get(&pick_p) {
                if let Some((ts_next, ui_next)) = q.front() {
                    let key = (*ts_next, pick_p, ui_next.offset);
                    heap.push((Reverse(key), pick_p));
                } else {
                    need_refill = true;
                }
            } else {
                need_refill = true;
            }
        }

        if need_refill {
            let mut local_idle = 0;
            loop {
                let done_now = { kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?.contains(&pick_p) };
                if done_now { break; }
                {
                    let bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                    if let Some(q) = bufs.get(&pick_p) { if q.front().is_some() { break; } }
                }
                if local_idle >= 5 { break; }
                match kafka.consumer.as_ref().poll(Duration::from_millis(200)) {
                    Some(Ok(m)) => {
                        let partition = m.partition();
                        let offset = m.offset();
                        let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                        if offset >= end {
                            let mut done = kafka.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                            done.insert(partition);
                        } else {
                            let (key_s, payload, decoding_error) = kafka.decode(m.key(), m.payload());
                            let (ts_ms, ts_str) = match m.timestamp() {
                                rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                                rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                                    if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                                }
                            };
                            let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key: key_s, offset, message: payload, timestamp: ts_str, decoding_error };
                            let mut bufs = kafka.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                            let q = bufs.entry(partition).or_insert_with(VecDeque::new);
                            let was_empty = q.is_empty();
                            q.push_back((ts_ms, ui));
                            drop(bufs);
                            if was_empty {
                                heap.push((Reverse((ts_ms, partition, offset)), partition));
                            }
                            if partition == pick_p { break; }
                        }
                    }
                    Some(Err(_)) | None => { local_idle += 1; }
                }
            }
        }
    }

    Ok(out)
}
