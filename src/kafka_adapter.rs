use tauri::State;
use serde::Deserialize;

use crate::app::AppState;
use crate::kafka::{Kafka, KafkaConfig, UiMessage};

/// Arguments for applying simple filters from the UI.
/// - partition: "all" or specific partition as string
/// - start_offset: starting offset when a specific partition is selected
#[derive(Debug, Deserialize)]
pub struct ApplyFiltersArgs {
    pub partition: Option<String>,
    #[serde(rename = "start_offset", alias = "startOffset")]
    pub start_offset: Option<i64>,
}

/// Configure Kafka connection (invoked from UI). This (re)creates a consumer.
#[tauri::command]
pub async fn set_kafka_config(state: State<'_, AppState>, config: KafkaConfig) -> Result<(), String> {
    state
        .reconfigure_kafka(config)
        .map_err(|e| format!("Failed to configure Kafka: {e}"))
}

/// Read-only status for the UI header.
#[tauri::command]
pub fn get_kafka_status(state: State<AppState>) -> Result<String, String> {
    let guard = state.kafka.lock().map_err(|e| format!("Failed to access state: {e}"))?;
    if let Some(k) = &*guard {
        Ok(format!("connected to {} topic {}", k.config.broker, k.config.topic))
    } else {
        Err("Kafka is not configured".to_string())
    }
}

/// List topics for a given broker.
#[tauri::command]
pub async fn get_topics(config: KafkaConfig) -> Result<Vec<String>, String> {
    Kafka::list_topics(&config).map_err(|e| format!("Failed to get topics: {e}"))
}

/// List partitions for the selected topic.
#[tauri::command]
pub async fn get_topic_partitions(config: KafkaConfig) -> Result<Vec<i32>, String> {
    Kafka::topic_partitions(&config).map_err(|e| format!("Failed to get partitions: {e}"))
}

/// Apply filters (partition/offset). Resets internal reading state.
#[tauri::command]
pub async fn apply_filters(
    state: State<'_, AppState>,
    args: ApplyFiltersArgs,
) -> Result<(), String> {
    let mut guard = state.kafka.lock().map_err(|e| format!("Failed to access state: {e}"))?;
    if let Some(k) = guard.as_mut() {
        k.apply_filters_mut(args.partition, args.start_offset)
            .map_err(|e| format!("Failed to apply filters: {e}"))
    } else {
        Err("Kafka is not configured".into())
    }
}

/// Consume the next batch of messages using the currently selected strategy.
#[tauri::command]
pub async fn consume_next_messages(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<UiMessage>, String> {
    let guard = state.kafka.lock().map_err(|e| format!("Failed to access state: {e}"))?;
    if let Some(k) = &*guard {
        let lim = limit.unwrap_or(200);
        k.consume_next(lim).map_err(|e| format!("Failed to consume messages: {e}"))
    } else {
        Err("Kafka is not configured".into())
    }
}
