use std::time::Duration;

use rdkafka::consumer::Consumer;

use super::consumer::create_consumer;
use super::types::KafkaConfig;

impl super::service::Kafka {
    /// Discover all topics in the cluster.
    pub fn list_topics(config: &KafkaConfig) -> anyhow::Result<Vec<String>> {
        let consumer = create_consumer(config)?;
        let md = consumer
            .client()
            .fetch_metadata(None, Duration::from_secs(5))?;
        let mut names: Vec<String> = md.topics().iter().map(|t| t.name().to_string()).collect();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Get partitions for a specific topic.
    pub fn topic_partitions(config: &KafkaConfig) -> anyhow::Result<Vec<i32>> {
        let consumer = create_consumer(config)?;
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
}
