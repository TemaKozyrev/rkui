use std::sync::{Arc, Mutex};

use crate::kafka::{Kafka, KafkaConfig};

/// Cancellation session for an in-flight streaming load.
#[derive(Clone)]
pub struct LoadSession {
    pub cancel_tx: tokio::sync::broadcast::Sender<()>,
}

/// Global application state shared with Tauri commands.
#[derive(Clone)]
pub struct AppState {
    /// Kafka reader instance; None until configured from the UI.
    pub kafka: Arc<Mutex<Option<Kafka>>>,
    /// Current streaming load session (if any).
    pub load_session: Arc<Mutex<Option<LoadSession>>>,
}

impl AppState {
    /// Construct an empty application state.
    pub fn new() -> Self {
        Self {
            kafka: Arc::new(Mutex::new(None)),
            load_session: Arc::new(Mutex::new(None)),
        }
    }

    /// Create/replace Kafka reader according to new config.
    pub fn reconfigure_kafka(&self, cfg: KafkaConfig) -> anyhow::Result<()> {
        let mut guard = self.kafka.lock().map_err(|e| anyhow::anyhow!("Failed to access state: {e}"))?;
        // Drop previous (it will close on drop)
        *guard = None;
        let kafka = Kafka::new(cfg)?;
        *guard = Some(kafka);
        Ok(())
    }
}
