use std::sync::{Arc, Mutex};

use crate::kafka::{Kafka, KafkaConfig};

#[derive(Clone)]
pub struct AppState {
    pub kafka: Arc<Mutex<Option<Kafka>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            kafka: Arc::new(Mutex::new(None)),
        }
    }

    pub fn reconfigure_kafka(&self, cfg: KafkaConfig) -> anyhow::Result<()> {
        let mut guard = self.kafka.lock().unwrap();
        // Drop previous (it will close on drop)
        *guard = None;
        let kafka = Kafka::new(cfg)?;
        *guard = Some(kafka);
        Ok(())
    }
}
