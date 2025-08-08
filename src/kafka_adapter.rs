use tauri::State;

use crate::app::AppState;
use crate::kafka::{Kafka, KafkaConfig};

#[tauri::command]
pub async fn set_kafka_config(state: State<'_, AppState>, config: KafkaConfig) -> Result<(), String> {
    state
        .reconfigure_kafka(config)
        .map_err(|e| format!("Failed to configure Kafka: {e}"))
}

#[tauri::command]
pub fn get_kafka_status(state: State<AppState>) -> String {
    let guard = state.kafka.lock().unwrap();
    if let Some(k) = &*guard {
        format!("connected to {} topic {}", k.config.broker, k.config.topic)
    } else {
        "not configured".to_string()
    }
}

#[tauri::command]
pub async fn get_topics(config: KafkaConfig) -> Result<Vec<String>, String> {
    Kafka::list_topics(&config).map_err(|e| format!("Failed to get topics: {e}"))
}
