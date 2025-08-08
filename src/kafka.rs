use std::collections::{HashMap, HashSet, VecDeque, BinaryHeap};
use std::cmp::Reverse;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::message::Message as RdMessage;
use rdkafka::topic_partition_list::TopicPartitionList;
use rdkafka::Offset;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMessage {
    pub id: String,
    pub partition: i32,
    pub key: String,
    pub offset: i64,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "json")] Json,
    #[serde(rename = "text")] Text,
    #[serde(rename = "protobuf")] Protobuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub broker: String,
    pub topic: String,
    pub ssl_enabled: bool,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    pub ssl_ca_path: Option<String>,
    pub message_type: MessageType,
    pub partition: Option<String>,
    pub start_offset: Option<i64>,
    pub proto_schema_path: Option<String>,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            broker: "localhost:9092".into(),
            topic: "".into(),
            ssl_enabled: false,
            ssl_cert_path: None,
            ssl_key_path: None,
            ssl_ca_path: None,
            message_type: MessageType::Json,
            partition: None,
            start_offset: None,
            proto_schema_path: None,
        }
    }
}

pub struct Kafka {
    pub config: KafkaConfig,
    pub consumer: Arc<BaseConsumer>,
    pub assigned: AtomicBool,
    // Snapshot of end offsets (high watermarks) per partition at configuration time
    pub end_offsets: Mutex<HashMap<i32, i64>>,
    // List of partitions we read from
    pub partitions: Mutex<Vec<i32>>,
    // Partitions that reached their end (as of the snapshot)
    pub done_partitions: Mutex<HashSet<i32>>,
    // Per-partition buffered messages to support global timestamp ordering and pagination
    pub buffers: Mutex<HashMap<i32, VecDeque<(i64, UiMessage)>>>, 
}

impl Kafka {
    pub fn create_consumer(config: &KafkaConfig) -> anyhow::Result<BaseConsumer> {
        let mut cc = ClientConfig::new();
        cc.set("bootstrap.servers", &config.broker);
        // A default group id; for UI reading anything is fine. Could be made configurable later.
        cc.set("group.id", "rkui-consumer");
        cc.set("enable.partition.eof", "false");
        cc.set("enable.auto.commit", "true");
        cc.set("auto.offset.reset", "earliest");

        if config.ssl_enabled {
            cc.set("security.protocol", "ssl");
            if let Some(path) = &config.ssl_ca_path {
                cc.set("ssl.ca.location", path);
            }
            if let Some(path) = &config.ssl_cert_path {
                cc.set("ssl.certificate.location", path);
            }
            if let Some(path) = &config.ssl_key_path {
                cc.set("ssl.key.location", path);
            }
        }

        let consumer: BaseConsumer = cc.create()?;
        Ok(consumer)
    }

    pub fn new(config: KafkaConfig) -> anyhow::Result<Self> {
        let consumer = Self::create_consumer(&config)?;
        Ok(Self {
            config,
            consumer: Arc::new(consumer),
            assigned: AtomicBool::new(false),
            end_offsets: Mutex::new(HashMap::new()),
            partitions: Mutex::new(Vec::new()),
            done_partitions: Mutex::new(HashSet::new()),
            buffers: Mutex::new(HashMap::new()),
        })
    }
}


impl Kafka {
    pub fn list_topics(config: &KafkaConfig) -> anyhow::Result<Vec<String>> {
        let consumer = Self::create_consumer(config)?;
        let md = consumer
            .client()
            .fetch_metadata(None, Duration::from_secs(5))?;
        let mut names: Vec<String> = md
            .topics()
            .iter()
            .map(|t| t.name().to_string())
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }

    pub fn topic_partitions(config: &KafkaConfig) -> anyhow::Result<Vec<i32>> {
        let consumer = Self::create_consumer(config)?;
        let md = consumer
            .client()
            .fetch_metadata(Some(&config.topic), Duration::from_secs(5))?;
        let t = md
            .topics()
            .iter()
            .find(|t| t.name() == config.topic)
            .ok_or_else(|| anyhow::anyhow!("Topic not found in metadata"))?;
        Ok(t.partitions().iter().map(|p| p.id()).collect())
    }

    pub fn apply_filters_mut(&mut self, partition: Option<String>, start_offset: Option<i64>) -> anyhow::Result<()> {
        // Update config with new filters
        self.config.partition = partition;
        self.config.start_offset = start_offset;
        // Reset assignment state so next consume will reassign
        self.assigned.store(false, Ordering::SeqCst);
        {
            let mut ends = self.end_offsets.lock().unwrap();
            ends.clear();
        }
        {
            let mut parts = self.partitions.lock().unwrap();
            parts.clear();
        }
        {
            let mut done = self.done_partitions.lock().unwrap();
            done.clear();
        }
        // Clear buffers and current assignment
        {
            let mut bufs = self.buffers.lock().unwrap();
            bufs.clear();
        }
        let empty = TopicPartitionList::new();
        self.consumer.assign(&empty)?;
        Ok(())
    }

    fn ensure_assigned(&self) -> anyhow::Result<()> {
        if self.assigned.swap(true, Ordering::SeqCst) {
            // already assigned
            return Ok(());
        }
        let topic = &self.config.topic;
        // Determine partitions to consume
        let partitions: Vec<i32> = if let Some(part_str) = &self.config.partition {
            if part_str != "all" && !part_str.is_empty() {
                vec![part_str.parse().unwrap_or(0)]
            } else {
                // enumerate all partitions for topic
                let md = self
                    .consumer
                    .client()
                    .fetch_metadata(Some(topic), Duration::from_secs(5))?;
                let t = md.topics().iter().find(|t| t.name() == topic).ok_or_else(|| anyhow::anyhow!("Topic not found in metadata"))?;
                t.partitions().iter().map(|p| p.id()).collect()
            }
        } else {
            // enumerate all partitions for topic
            let md = self
                .consumer
                .client()
                .fetch_metadata(Some(topic), Duration::from_secs(5))?;
            let t = md.topics().iter().find(|t| t.name() == topic).ok_or_else(|| anyhow::anyhow!("Topic not found in metadata"))?;
            t.partitions().iter().map(|p| p.id()).collect()
        };

        // Snapshot end offsets (high watermarks) before we start reading
        {
            let mut ends = self.end_offsets.lock().unwrap();
            ends.clear();
            for p in &partitions {
                let (_low, high) = self
                    .consumer
                    .fetch_watermarks(topic, *p, Duration::from_secs(5))?;
                ends.insert(*p, high);
            }
        }
        // store partitions
        {
            let mut parts = self.partitions.lock().unwrap();
            *parts = partitions.clone();
        }
        // init buffers for partitions
        {
            let mut bufs = self.buffers.lock().unwrap();
            bufs.clear();
        }

        // Assign explicit starting offsets based on selected partition and requested start_offset
        let mut tpl = TopicPartitionList::new();
        let is_all = self.config.partition.as_deref().map(|s| s == "all").unwrap_or(true);
        for p in partitions {
            let off = if is_all {
                // When reading all partitions, ignore start_offset and begin from earliest for each
                Offset::Beginning
            } else {
                if let Some(req) = self.config.start_offset {
                    // Clamp to earliest available if requested offset is older than retention (deleted)
                    let (low, _high) = self.consumer.fetch_watermarks(topic, p, Duration::from_secs(5))?;
                    let effective = if req < low { low } else { req };
                    Offset::Offset(effective)
                } else {
                    Offset::Beginning
                }
            };
            tpl.add_partition_offset(topic, p, off)?;
        }
        self.consumer.assign(&tpl)?;
        Ok(())
    }

    pub fn consume_next(&self, limit: usize) -> anyhow::Result<Vec<UiMessage>> {
        self.ensure_assigned()?;
        let ends = self.end_offsets.lock().unwrap().clone();
        let parts = self.partitions.lock().unwrap().clone();
        // If already done on all partitions, return immediately
        {
            let done = self.done_partitions.lock().unwrap();
            if parts.iter().all(|p| done.contains(p)) {
                return Ok(Vec::new());
            }
        }

        // If single partition is selected (not "all"), we can just stream in consumer order
        let partitions_all = self.config.partition.as_deref().map(|s| s == "all").unwrap_or(true);
        let mut out: Vec<UiMessage> = Vec::with_capacity(limit);

        if !partitions_all || parts.len() <= 1 {
            // Fallback to simple sequential consumption
            let mut idle_loops = 0;
            while out.len() < limit && idle_loops < 20 {
                match self.consumer.as_ref().poll(Duration::from_millis(200)) {
                    Some(Ok(m)) => {
                        let partition = m.partition();
                        let offset = m.offset();
                        let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                        if offset >= end {
                            let mut done = self.done_partitions.lock().unwrap();
                            done.insert(partition);
                            if parts.iter().all(|p| done.contains(p)) { break; }
                            continue;
                        }
                        let key = m.key().map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
                        let payload = m.payload().map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
                        let timestamp = match m.timestamp() {
                            rdkafka::message::Timestamp::NotAvailable => String::from(""),
                            rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                                if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { dt.to_rfc3339() } else { String::new() }
                            }
                        };
                        out.push(UiMessage { id: format!("{}-{}", partition, offset), partition, key, offset, message: payload, timestamp });
                        if offset >= end - 1 {
                            let mut done = self.done_partitions.lock().unwrap();
                            done.insert(partition);
                            if parts.iter().all(|p| done.contains(p)) { break; }
                        }
                    }
                    Some(Err(_)) | None => { idle_loops += 1; }
                }
            }
            return Ok(out);
        }

        // Multi-partition global ordering by timestamp using per-partition buffers
        let mut idle_loops = 0;
        // Step 1: try to ensure each active partition has at least one buffered message
        loop {
            // Compute which partitions are still active and need buffering
            let done = self.done_partitions.lock().unwrap().clone();
            let mut bufs = self.buffers.lock().unwrap();
            let mut need = false;
            for &p in &parts {
                if done.contains(&p) { continue; }
                let e = bufs.entry(p).or_insert_with(VecDeque::new);
                if e.front().is_none() { need = true; }
            }
            drop(bufs);
            if !need { break; }
            if idle_loops >= 20 { break; }
            match self.consumer.as_ref().poll(Duration::from_millis(200)) {
                Some(Ok(m)) => {
                    let partition = m.partition();
                    let offset = m.offset();
                    let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                    if offset >= end {
                        let mut done = self.done_partitions.lock().unwrap();
                        done.insert(partition);
                        continue;
                    }
                    let key = m.key().map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
                    let payload = m.payload().map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
                    let (ts_ms, ts_str) = match m.timestamp() {
                        rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                        rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                            if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                        }
                    };
                    let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key, offset, message: payload, timestamp: ts_str };
                    let mut bufs = self.buffers.lock().unwrap();
                    bufs.entry(partition).or_insert_with(VecDeque::new).push_back((ts_ms, ui));
                    if offset >= end - 1 {
                        let mut done = self.done_partitions.lock().unwrap();
                        done.insert(partition);
                    }
                }
                Some(Err(_)) | None => { idle_loops += 1; }
            }
        }

        // Step 2: merge-pop by minimal timestamp using a min-heap (priority queue)
        let mut heap: BinaryHeap<(Reverse<(i64, i32, i64)>, i32)> = BinaryHeap::new();
        {
            // Initialize heap with current heads
            let bufs = self.buffers.lock().unwrap();
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
                match self.consumer.as_ref().poll(Duration::from_millis(200)) {
                    Some(Ok(m)) => {
                        let partition = m.partition();
                        let offset = m.offset();
                        let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                        if offset >= end {
                            let mut done = self.done_partitions.lock().unwrap();
                            done.insert(partition);
                        } else {
                            let key_s = m.key().map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
                            let payload = m.payload().map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
                            let (ts_ms, ts_str) = match m.timestamp() {
                                rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                                rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                                    if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                                }
                            };
                            let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key: key_s, offset, message: payload, timestamp: ts_str };
                            let mut bufs = self.buffers.lock().unwrap();
                            let q = bufs.entry(partition).or_insert_with(VecDeque::new);
                            let was_empty = q.is_empty();
                            q.push_back((ts_ms, ui));
                            drop(bufs);
                            if was_empty {
                                // newly available head -> push to heap
                                heap.push((Reverse((ts_ms, partition, offset)), partition));
                            }
                        }
                    }
                    Some(Err(_)) | None => { idle_loops += 1; }
                }
                continue;
            }

            // Pop the smallest timestamp item (by partition), then actually pop from that partition buffer
            let (_key, pick_p) = heap.pop().unwrap();
            let maybe_item = {
                let mut bufs = self.buffers.lock().unwrap();
                if let Some(q) = bufs.get_mut(&pick_p) { q.pop_front() } else { None }
            };
            let Some((ts_emitted, ui)) = maybe_item else {
                // nothing to emit for this partition, continue
                continue;
            };
            let _ = ts_emitted; // used for ordering only
            let end_for_p = ends.get(&pick_p).cloned().unwrap_or(i64::MAX);
            if ui.offset >= end_for_p {
                let mut done = self.done_partitions.lock().unwrap();
                done.insert(pick_p);
            }
            if ui.offset >= end_for_p - 1 {
                let mut done = self.done_partitions.lock().unwrap();
                done.insert(pick_p);
            }
            out.push(ui);

            // Push next head for this partition if available; otherwise try to refill specifically this partition
            let mut need_refill = false;
            {
                let bufs = self.buffers.lock().unwrap();
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
                // Attempt to poll until we get at least one message for pick_p or reach limits
                let mut local_idle = 0;
                loop {
                    let done_now = { self.done_partitions.lock().unwrap().contains(&pick_p) };
                    if done_now { break; }
                    {
                        let bufs = self.buffers.lock().unwrap();
                        if let Some(q) = bufs.get(&pick_p) { if q.front().is_some() { break; } }
                    }
                    if local_idle >= 5 { break; }
                    match self.consumer.as_ref().poll(Duration::from_millis(200)) {
                        Some(Ok(m)) => {
                            let partition = m.partition();
                            let offset = m.offset();
                            let end = ends.get(&partition).cloned().unwrap_or(i64::MAX);
                            if offset >= end {
                                let mut done = self.done_partitions.lock().unwrap();
                                done.insert(partition);
                            } else {
                                let key_s = m.key().map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
                                let payload = m.payload().map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
                                let (ts_ms, ts_str) = match m.timestamp() {
                                    rdkafka::message::Timestamp::NotAvailable => (i64::MAX, String::from("")),
                                    rdkafka::message::Timestamp::CreateTime(ms) | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                                        if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) { (ms, dt.to_rfc3339()) } else { (ms, String::new()) }
                                    }
                                };
                                let ui = UiMessage { id: format!("{}-{}", partition, offset), partition, key: key_s, offset, message: payload, timestamp: ts_str };
                                let mut bufs = self.buffers.lock().unwrap();
                                let q = bufs.entry(partition).or_insert_with(VecDeque::new);
                                let was_empty = q.is_empty();
                                q.push_back((ts_ms, ui));
                                drop(bufs);
                                if was_empty {
                                    // if this was the target partition, push it now; otherwise, its head was missing so we add it as well
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
}
