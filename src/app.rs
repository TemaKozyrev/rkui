use std::sync::{Arc, Mutex};

use crate::kafka::{Kafka, KafkaConfig};

/// Global application state shared with Tauri commands.
#[derive(Clone)]
pub struct AppState {
    /// Kafka reader instance; None until configured from the UI.
    pub kafka: Arc<Mutex<Option<Kafka>>>,
}

impl AppState {
    /// Construct an empty application state.
    pub fn new() -> Self {
        Self {
            kafka: Arc::new(Mutex::new(None)),
        }
    }

    /// Create/replace Kafka reader according to new config.
    pub fn reconfigure_kafka(&self, cfg: KafkaConfig) -> anyhow::Result<()> {
        let mut guard = self.kafka.lock().unwrap();
        // Drop previous (it will close on drop)
        *guard = None;
        let kafka = Kafka::new(cfg)?;
        *guard = Some(kafka);
        Ok(())
    }
}
