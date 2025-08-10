use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::topic_partition_list::TopicPartitionList;
use rdkafka::Offset;
use serde::{Deserialize, Serialize};

mod decoder;
pub mod reader;

pub use decoder::{MessageDecoder, MessageType, decoder_for};

/// UI-facing message representation. Keep it small and serializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMessage {
    pub id: String,
    pub partition: i32,
    pub key: String,
    pub offset: i64,
    pub message: String,
    pub timestamp: String,
    pub decoding_error: Option<String>,
}

/// Kafka connection and reading configuration coming from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub broker: String,
    pub topic: String,
    // Legacy flag kept for backward compatibility with older UIs
    pub ssl_enabled: bool,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    pub ssl_ca_path: Option<String>,
    /// Optional security type sent by the UI: "plaintext" | "ssl" | "sasl_plaintext"
    #[serde(rename = "security_type", alias = "securityType")]
    pub security_type: Option<String>,
    /// Optional SASL mechanism (e.g., PLAIN, SCRAM-SHA-256, SCRAM-SHA-512)
    #[serde(rename = "sasl_mechanism", alias = "saslMechanism")]
    pub sasl_mechanism: Option<String>,
    /// JAAS-like config string; we will parse username/password out of it
    #[serde(rename = "sasl_jaas_config", alias = "saslJaasConfig")]
    pub sasl_jaas_config: Option<String>,
    pub message_type: MessageType,
    /// "all" or a specific partition id as string
    pub partition: Option<String>,
    /// Starting offset for a specific partition (ignored when partition == "all")
    pub start_offset: Option<i64>,
    /// Start position preference: "oldest" (default) or "newest"
    #[serde(rename = "start_from", alias = "startFrom")]
    pub start_from: Option<String>,
    /// Optional path to proto schema (future use)
    pub proto_schema_path: Option<String>,
    /// Optional fully qualified proto message name selected in UI
    #[serde(
        rename = "proto_message_full_name",
        alias = "protoMessageFullName",
        alias = "message_full_name",
        alias = "messageFullName"
    )]
    pub proto_message_full_name: Option<String>,
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
            security_type: None,
            sasl_mechanism: None,
            sasl_jaas_config: None,
            message_type: MessageType::Json,
            partition: None,
            start_offset: None,
            start_from: Some("oldest".into()),
            proto_schema_path: None,
            proto_message_full_name: None,
        }
    }
}

/// Try to extract username and password from a JAAS-like config string.
/// Expected patterns include username="user" password="pass" (quotes can be ' or ").
fn parse_username_password_from_jaas(s: &str) -> Option<(String, String)> {
    fn extract(field: &str, s: &str) -> Option<String> {
        let needle = format!("{}=", field);
        let idx = s.find(&needle)?;
        let after = &s[idx + needle.len()..];
        match after.chars().next()? {
            '"' | '\'' => { /* handled below */ }
            _ => {
                // Unquoted value: read until whitespace or semicolon
                let end = after.find(|ch: char| ch.is_whitespace() || ch == ';').unwrap_or(after.len());
                return Some(after[..end].to_string());
            }
        }
        // We got the first quote char in 'after', need to actually scan correctly
        let first = after.chars().next()?;
        if first == '"' || first == '\'' {
            let q = first;
            let rest = &after[first.len_utf8()..];
            if let Some(end) = rest.find(q) {
                return Some(rest[..end].to_string());
            }
        }
        None
    }
    let user = extract("username", s)?;
    let pass = extract("password", s)?;
    Some((user, pass))
}

/// High-level Kafka reader object. Encapsulates consumer and reading state.
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
    // Optional protobuf decoder initialized when proto schema path is provided
    pub proto_decoder: Option<Arc<crate::proto_decoder::ProtoDecoder>>,
}

impl Kafka {
    /// Create a configured rdkafka consumer.
    pub fn create_consumer(config: &KafkaConfig) -> anyhow::Result<BaseConsumer> {
        let mut cc = ClientConfig::new();
        cc.set("bootstrap.servers", &config.broker);
        // A default group id; for UI reading anything is fine. Could be made configurable later.
        cc.set("group.id", "rkui-consumer");
        cc.set("enable.partition.eof", "false");
        cc.set("enable.auto.commit", "true");
        cc.set("auto.offset.reset", "earliest");

        // Determine effective security type with backward compatibility
        let sec_type = config
            .security_type
            .as_deref()
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| if config.ssl_enabled { "ssl".into() } else { "plaintext".into() });

        match sec_type.as_str() {
            "ssl" => {
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
            "sasl_plaintext" => {
                cc.set("security.protocol", "sasl_plaintext");
                if let Some(mech) = &config.sasl_mechanism {
                    cc.set("sasl.mechanism", mech);
                }
                // Prefer explicit username/password parsed from JAAS config string
                if let Some(jaas) = &config.sasl_jaas_config {
                    if let Some((user, pass)) = parse_username_password_from_jaas(jaas) {
                        cc.set("sasl.username", &user);
                        cc.set("sasl.password", &pass);
                    }
                }
            }
            _ => {
                // plaintext (default): no extra settings
            }
        }

        let consumer: BaseConsumer = cc.create()?;
        Ok(consumer)
    }

    /// Construct a Kafka object with empty state.
    pub fn new(config: KafkaConfig) -> anyhow::Result<Self> {
        let consumer = Self::create_consumer(&config)?;
        // Initialize proto decoder if requested
        let proto_decoder = if matches!(config.message_type, MessageType::Protobuf) {
            // In protobuf mode, schema path must be provided and decoder must initialize successfully
            let path = config
                .proto_schema_path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Protobuf message_type selected but no proto_schema_path provided"))?;
            match crate::proto_decoder::ProtoDecoder::from_proto_files(vec![path.clone()], config.proto_message_full_name.clone()) {
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
        let key_s = key.map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
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
}

impl Kafka {
    /// Discover all topics in the cluster.
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

    /// Get partitions for a specific topic.
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

    /// Apply partition/offset filters and reset internal reading state.
    pub fn apply_filters_mut(&mut self, partition: Option<String>, start_offset: Option<i64>, start_from: Option<String>) -> anyhow::Result<()> {
        self.config.partition = partition;
        self.config.start_offset = start_offset;
        self.config.start_from = start_from.or_else(|| self.config.start_from.clone());
        // Reset assignment state so next consume will reassign
        self.assigned.store(false, Ordering::SeqCst);
        self.end_offsets.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (end_offsets): {e}"))?.clear();
        self.partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (partitions): {e}"))?.clear();
        self.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?.clear();
        self.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?.clear();
        let empty = TopicPartitionList::new();
        self.consumer.assign(&empty)?;
        Ok(())
    }

    /// Ensure we are assigned to the desired partitions with proper starting offsets.
    pub(crate) fn ensure_assigned(&self) -> anyhow::Result<()> {
        if self.assigned.swap(true, Ordering::SeqCst) {
            return Ok(()); // already assigned
        }
        let topic = &self.config.topic;
        // Determine partitions to consume
        let partitions: Vec<i32> = if let Some(part_str) = &self.config.partition {
            if part_str != "all" && !part_str.is_empty() {
                let p: i32 = part_str.parse().map_err(|e| anyhow::anyhow!("Invalid partition id '{}': {}", part_str, e))?;
                vec![p]
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
            let mut ends = self.end_offsets.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (end_offsets): {e}"))?;
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
            let mut parts = self.partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (partitions): {e}"))?;
            *parts = partitions.clone();
        }
        // init buffers for partitions
        self.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?.clear();

        // Assign explicit starting offsets based on selected partition and requested start_offset
        let mut tpl = TopicPartitionList::new();
        let is_all = self.config.partition.as_deref().map(|s| s == "all").unwrap_or(true);
        let newest = self.config.start_from.as_deref().map(|s| s.eq_ignore_ascii_case("newest")).unwrap_or(false);
        const BACK_WINDOW: i64 = 2000; // how many latest offsets to read back from end when starting from newest
        for p in partitions {
            let off = if newest {
                let (low, high) = self.consumer.fetch_watermarks(topic, p, Duration::from_secs(5))?;
                let start = if high > BACK_WINDOW { high - BACK_WINDOW } else { low };
                Offset::Offset(start)
            } else if is_all {
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

    /// Read next batch of messages according to the selected strategy.
    pub fn consume_next(&self, limit: usize) -> anyhow::Result<Vec<UiMessage>> {
        self.ensure_assigned()?;
        let ends = self.end_offsets.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (end_offsets): {e}"))?.clone();
        let parts = self.partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (partitions): {e}"))?.clone();
        // If all partitions are marked done, return only when internal buffers are fully drained.
        {
            let all_done = {
                let done = self.done_partitions.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (done_partitions): {e}"))?;
                parts.iter().all(|p| done.contains(p))
            };
            if all_done {
                let has_buffered = {
                    let bufs = self.buffers.lock().map_err(|e| anyhow::anyhow!("State lock poisoned (buffers): {e}"))?;
                    bufs.values().any(|q| !q.is_empty())
                };
                if !has_buffered {
                    return Ok(Vec::new());
                }
            }
        }

        let partitions_all = self.config.partition.as_deref().map(|s| s == "all").unwrap_or(true);
        if !partitions_all || parts.len() <= 1 {
            return reader::consume_sequential(self, &ends, &parts, limit);
        }
        reader::consume_merge(self, &ends, &parts, limit)
    }
}
