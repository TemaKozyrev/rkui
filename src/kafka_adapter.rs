use tauri::State;
use serde::Deserialize;

use crate::app::AppState;
use crate::kafka::{Kafka, KafkaConfig, UiMessage};

#[derive(Debug, Deserialize)]
pub struct ApplyFiltersArgs {
    pub partition: Option<String>,
    #[serde(rename = "start_offset", alias = "startOffset")]
    pub start_offset: Option<i64>,
}

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

#[tauri::command]
pub async fn get_topic_partitions(config: KafkaConfig) -> Result<Vec<i32>, String> {
    Kafka::topic_partitions(&config).map_err(|e| format!("Failed to get partitions: {e}"))
}

#[tauri::command]
pub async fn apply_filters(
    state: State<'_, AppState>,
    args: ApplyFiltersArgs,
) -> Result<(), String> {
    let mut guard = state.kafka.lock().unwrap();
    if let Some(k) = guard.as_mut() {
        k.apply_filters_mut(args.partition, args.start_offset)
            .map_err(|e| format!("Failed to apply filters: {e}"))
    } else {
        Err("Kafka is not configured".into())
    }
}

#[tauri::command]
pub async fn consume_next_messages(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<UiMessage>, String> {
    let guard = state.kafka.lock().unwrap();
    if let Some(k) = &*guard {
        let lim = limit.unwrap_or(200);
        k.consume_next(lim).map_err(|e| format!("Failed to consume messages: {e}"))
    } else {
        Err("Kafka is not configured".into())
    }
}
