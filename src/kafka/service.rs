use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::decoder::{decoder_for, MessageType};
use super::reader;
use super::types::{KafkaConfig, UiMessage};

/// High-level Kafka reader object. Encapsulates consumer and reading state.
pub struct Kafka {
    pub config: KafkaConfig,
    pub consumer: Arc<rdkafka::consumer::BaseConsumer>,
    pub assigned: AtomicBool,
    // Snapshot of end offsets (high watermarks) per partition at configuration time
    pub end_offsets: Mutex<HashMap<i32, i64>>,
    // List of partitions we read from
    pub partitions: Mutex<Vec<i32>>,
    // Partitions that reached their end (as of the snapshot)
    pub done_partitions: Mutex<HashSet<i32>>,
    // Per-partition buffered messages to support global timestamp ordering and pagination
    pub buffers: Mutex<HashMap<i32, VecDeque<(i64, UiMessage)>>>,
    // Optional protobuf decoder initialized when proto schema path is provided
    pub proto_decoder: Option<Arc<crate::proto_decoder::ProtoDecoder>>,
}

impl Kafka {
    /// Construct a Kafka object with empty state.
    pub fn new(config: KafkaConfig) -> anyhow::Result<Self> {
        let consumer = super::consumer::create_consumer(&config)?;
        // Initialize proto decoder if requested
        let proto_decoder = if matches!(config.message_type, MessageType::Protobuf) {
            // In protobuf mode, schema path must be provided and decoder must initialize successfully
            let path = config
                .proto_schema_path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "Protobuf message_type selected but no proto_schema_path provided"
                ))?;
            match crate::proto_decoder::ProtoDecoder::from_proto_files(
                vec![path.clone()],
                config.proto_message_full_name.clone(),
            ) {
                Ok(dec) => Some(dec),
                Err(e) => {
                    // Convert initialization failure into a hard error so UI gets it
                    return Err(anyhow::anyhow!("Failed to initialize proto decoder: {}", e));
                }
            }
        } else {
            None
        };
        Ok(Self {
            config,
            consumer: Arc::new(consumer),
            assigned: AtomicBool::new(false),
            end_offsets: Mutex::new(HashMap::new()),
            partitions: Mutex::new(Vec::new()),
            done_partitions: Mutex::new(HashSet::new()),
            buffers: Mutex::new(HashMap::new()),
            proto_decoder,
        })
    }

    /// Lightweight helper that decodes key/value according to configured message type.
    pub fn decode(&self, key: Option<&[u8]>, payload: Option<&[u8]>) -> (String, String, Option<String>) {
        // Key as UTF-8 lossy
        let key_s = key
            .map(|k| String::from_utf8_lossy(k).to_string())
            .unwrap_or_default();
        // If protobuf configured and decoder available, try to decode to JSON
        if matches!(self.config.message_type, MessageType::Protobuf) {
            if let (Some(pd), Some(bytes)) = (self.proto_decoder.as_ref(), payload) {
                match pd.decode(bytes) {
                    Ok(json) => return (key_s, json, None),
                    Err(e) => {
                        // Failed to decode: return raw text and attach error, but do not stop reading
                        let raw = String::from_utf8_lossy(bytes).to_string();
                        return (key_s, raw, Some(format!("Protobuf decode error: {}", e)));
                    }
                }
            }
        }
        // Fallback to existing decoders
        let dec = decoder_for(&self.config.message_type);
        let (_k, v) = dec.decode(None, payload);
        (key_s, v, None)
    }

    /// Read next batch of messages according to the selected strategy.
    pub fn consume_next(&self, limit: usize) -> anyhow::Result<Vec<UiMessage>> {
        self.ensure_assigned()?;
        let ends = self
            .end_offsets
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (end_offsets): {e}"))?
            .clone();
        let parts = self
            .partitions
            .lock()
            .map_err(|e| anyhow::anyhow!("State lock poisoned (partitions): {e}"))?
            .clone();
        // If all partitions are marked done, return only when internal buffers are fully drained.
        {
            let all_done = {
                let done = self
                    .done_partitions
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                parts.iter().all(|p| done.contains(p))
            };
            if all_done {
                let has_buffered = {
                    let bufs = self
                        .buffers
                        .lock()
                        .map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                    bufs.values().any(|q| !q.is_empty())
                };
                if !has_buffered {
                    return Ok(Vec::new());
                }
            }
        }

        let partitions_all = self
            .config
            .partition
            .as_deref()
            .map(|s| s == "all")
            .unwrap_or(true);
        if !partitions_all || parts.len() <= 1 {
            return reader::consume_sequential(self, &ends, &parts, limit);
        }
        reader::consume_merge(self, &ends, &parts, limit)
    }
}
