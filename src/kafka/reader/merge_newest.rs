use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::time::Duration;

use rdkafka::message::Message as RdMessage;

use crate::kafka::{Kafka, UiMessage};

/// Newest-first merge across multiple partitions using buffered tails and a max-heap.
pub fn consume_merge_newest(
    kafka: &Kafka,
    ends: &HashMap<i32, i64>,
    parts: &Vec<i32>,
    limit: usize,
) -> anyhow::Result<Vec<UiMessage>> {
    // Prefill buffers with all available messages up to the snapshot end for each partition
    let mut idle_loops = 0;
    loop {
        let done_all = {
            let done = kafka
                .done_partitions
                .lock()
                .map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
            parts.iter().all(|p| done.contains(p))
        };
        if done_all { break; }
        if idle_loops >= 40 { break; }
        match kafka.consumer.as_ref().poll(Duration::from_millis(200)) {
            Some(Ok(m)) => {
                let partition = m.partition();
                let offset = m.offset();
                let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                if offset >= end {
                    let mut done = kafka
                        .done_partitions
                        .lock()
                        .map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                    done.insert(partition);
                } else {
                    let (key, payload, decoding_error) = kafka.decode(m.key(), m.payload());
                    let (ts_ms, ts_str) = match m.timestamp() {
                        rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                        rdkafka::message::Timestamp::CreateTime(ms)
                        | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                            if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                        }
                    };
                    let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key, offset, message: payload, timestamp: ts_str, decoding_error };
                    let mut bufs = kafka
                        .buffers
                        .lock()
                        .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                    bufs.entry(partition).or_insert_with(VecDeque::new).push_back((ts_ms, ui));
                    if offset >= end - 1 {
                        let mut done = kafka
                            .done_partitions
                            .lock()
                            .map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                        done.insert(partition);
                    }
                }
            }
            Some(Err(_)) | None => { idle_loops += 1; }
        }
    }

    // Emit newest-first using a max-heap of partition tails
    let mut out: Vec<UiMessage> = Vec::with_capacity(limit);
    let mut heap: BinaryHeap<((i64, i32, i64), i32)> = BinaryHeap::new();
    {
        let bufs = kafka
            .buffers
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
        for (&p, q) in bufs.iter() {
            if let Some((ts, ui)) = q.back() {
                let key = (*ts, p, ui.offset);
                heap.push((key, p));
            }
        }
    }
    while out.len() < limit {
        if let Some((_, pick_p)) = heap.pop() {
            let maybe_item = {
                let mut bufs = kafka
                    .buffers
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                if let Some(q) = bufs.get_mut(&pick_p) { q.pop_back() } else { None }
            };
            let Some((_ts_emitted, ui)) = maybe_item else { continue; };
            out.push(ui);
            let next_tail = {
                let bufs = kafka
                    .buffers
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                if let Some(q) = bufs.get(&pick_p) { q.back().map(|(ts, ui)| (*ts, pick_p, ui.offset)) } else { None }
            };
            if let Some(key) = next_tail { heap.push((key, pick_p)); }
        } else { break; }
    }
    Ok(out)
}
