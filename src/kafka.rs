use std::sync::Arc;
use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use serde::{Deserialize, Serialize};

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
    pub offset_type: Option<String>,
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
            offset_type: None,
            start_offset: None,
            proto_schema_path: None,
        }
    }
}

pub struct Kafka {
    pub config: KafkaConfig,
    pub consumer: Arc<StreamConsumer>,
}

impl Kafka {
    pub fn create_consumer(config: &KafkaConfig) -> anyhow::Result<StreamConsumer> {
        let mut cc = ClientConfig::new();
        cc.set("bootstrap.servers", &config.broker);
        // A default group id; for UI reading anything is fine. Could be made configurable later.
        cc.set("group.id", "rkui-consumer");
        cc.set("enable.partition.eof", "false");
        cc.set("enable.auto.commit", "true");
        cc.set("auto.offset.reset", match config.offset_type.as_deref() {
            Some("earliest") => "earliest",
            Some("latest") => "latest",
            _ => "latest",
        });

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

        let consumer: StreamConsumer = cc.create()?;
        Ok(consumer)
    }

    pub fn new(config: KafkaConfig) -> anyhow::Result<Self> {
        let consumer = Self::create_consumer(&config)?;
        Ok(Self {
            config,
            consumer: Arc::new(consumer),
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
}
