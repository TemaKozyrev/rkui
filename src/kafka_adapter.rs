use tauri::{State, Window, Emitter, AppHandle};
use serde::{Deserialize, Serialize};
use rdkafka::consumer::Consumer;

use crate::app::{AppState, LoadSession};
use crate::kafka::{Kafka, KafkaConfig, UiMessage};

/// Arguments for applying simple filters from the UI.
/// - partition: "all" or specific partition as string
/// - start_offset: starting offset when a specific partition is selected
#[derive(Debug, Deserialize)]
pub struct ApplyFiltersArgs {
    pub partition: Option<String>,
    #[serde(rename = "start_offset", alias = "startOffset")]
    pub start_offset: Option<i64>,
    #[serde(rename = "start_from", alias = "startFrom")]
    pub start_from: Option<String>,
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
        k.apply_filters_mut(args.partition, args.start_offset, args.start_from)
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

use tokio::sync::broadcast;

// jq/jaq support via jq-rs

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum FilterMode {
    #[serde(rename = "plain")] Plain,
    #[serde(rename = "jq")] Jq,
}

impl Default for FilterMode {
    fn default() -> Self { FilterMode::Plain }
}

#[derive(Debug, Deserialize)]
pub struct StartFilteredLoadArgs {
    pub limit: Option<usize>,
    #[serde(rename = "key_filter", alias = "keyFilter")]
    pub key_filter: Option<String>,
    #[serde(rename = "message_filter", alias = "messageFilter")]
    pub message_filter: Option<String>,
    #[serde(rename = "message_filter_mode", alias = "messageFilterMode")]
    pub message_filter_mode: Option<FilterMode>,
}


fn eval_jq_bool(program: &str, value: &serde_json::Value) -> Result<bool, String> {
    // Minimal jq-like evaluator without external/system dependencies.
    // Supported forms:
    // - ".a.b" -> true if value at path is boolean true
    // - ".a.b == <literal>" -> true if equal (literal parsed as JSON if possible, or as string)
    // - "true" -> true
    let src = program.trim();
    if src == "true" { return Ok(true); }

    // Split by '==' if present (simple, not handling complex jq syntax)
    if let Some(idx) = src.find("==") {
        let (left, right) = src.split_at(idx);
        let left = left.trim();
        let right = right.trim_start_matches("==").trim();
        let lv = json_path_get(value, left).ok_or_else(|| "jq path not found".to_string())?;
        // Try parse right as JSON literal
        let rv: serde_json::Value = match serde_json::from_str(right) {
            Ok(v) => v,
            Err(_) => {
                // strip optional surrounding quotes
                let r = right.trim();
                let r = r.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(r);
                serde_json::Value::String(r.to_string())
            }
        };
        return Ok(lv == rv);
    }

    // Path-only form
    if src.starts_with('.') {
        if let Some(v) = json_path_get(value, src) {
            return Ok(v == serde_json::Value::Bool(true));
        } else {
            return Ok(false);
        }
    }

    // Unsupported expression -> treat as non-match
    Err("unsupported jq expression".into())
}

fn json_path_get<'a>(root: &'a serde_json::Value, path: &str) -> Option<serde_json::Value> {
    // path like .a.b[0].c
    if !path.starts_with('.') { return None; }
    let mut cur = root;
    let mut idx = 1usize; // skip leading '.'
    while idx < path.len() {
        // parse key up to next '.' or '['
        let bytes = path.as_bytes();
        let mut j = idx;
        while j < bytes.len() && bytes[j] != b'.' && bytes[j] != b'[' { j += 1; }
        if j > idx {
            let key = &path[idx..j];
            cur = cur.get(key)?;
        }
        idx = j;
        if idx >= bytes.len() { break; }
        if bytes[idx] == b'.' { idx += 1; continue; }
        // handle [n]
        if bytes[idx] == b'[' {
            idx += 1;
            // read number
            let mut k = idx;
            while k < bytes.len() && bytes[k].is_ascii_digit() { k += 1; }
            if k == idx { return None; }
            let n: usize = path[idx..k].parse().ok()?;
            if k >= bytes.len() || bytes[k] != b']' { return None; }
            cur = cur.get(n)?;
            idx = k + 1;
            if idx < bytes.len() && bytes[idx] == b'.' { idx += 1; }
            continue;
        }
    }
    Some(cur.clone())
}

#[tauri::command]
pub async fn start_filtered_load(window: Window, state: State<'_, AppState>, args: StartFilteredLoadArgs) -> Result<(), String> {
    let limit = args.limit.unwrap_or(200);

    // Prepare Kafka access and snapshot necessary pieces
    let (consumer, message_type, proto_decoder, topic, parts, ends) = {
        let guard = state.kafka.lock().map_err(|e| format!("Failed to access state: {e}"))?;
        let Some(k) = &*guard else { return Err("Kafka is not configured".into()); };
        // Ensure assignment to requested partitions/offsets without consuming any messages
        if let Err(e) = k.ensure_assigned() {
            return Err(format!("Failed to assign consumer: {e}"));
        }
        let parts = k
            .partitions
            .lock()
            .map_err(|e| format!("State lock poisoned (partitions): {e}"))?
            .clone();
        let ends = k
            .end_offsets
            .lock()
            .map_err(|e| format!("State lock poisoned (end_offsets): {e}"))?
            .clone();
        (
            k.consumer.clone(),
            k.config.message_type.clone(),
            k.proto_decoder.clone(),
            k.config.topic.clone(),
            parts,
            ends,
        )
    };

    // Compute initial done set for empty partitions (low == end)
    let mut done_parts: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for p in &parts {
        if let Ok((low, _high)) = consumer.fetch_watermarks(&topic, *p, std::time::Duration::from_secs(5)) {
            if low >= *ends.get(p).unwrap_or(&i64::MAX) {
                done_parts.insert(*p);
            }
        }
    }

    // Cancel previous session if exists, then install a new one
    {
        let mut sess_guard = state.load_session.lock().map_err(|e| format!("Failed to access load session: {e}"))?;
        if let Some(prev) = sess_guard.take() {
            let _ = prev.cancel_tx.send(());
        }
        let (tx, _rx0) = broadcast::channel::<()>(1);
        *sess_guard = Some(LoadSession { cancel_tx: tx.clone() });
        drop(sess_guard);

        // Snapshot filter settings
        let filter_mode = args.message_filter_mode.unwrap_or(FilterMode::Plain);
        let key_filter = args.key_filter.clone();
        let msg_filter = args.message_filter.clone();

        // Emit started event
        let _ = window.emit("kafka:load_started", &serde_json::json!({
            "limit": limit,
            "keyFilter": key_filter,
            "messageFilter": msg_filter,
            "messageFilterMode": filter_mode,
        }));

        let mut rx = tx.subscribe();
        let win = window.clone();
        let mut done_parts_local = done_parts.clone();
        tokio::spawn(async move {
            use rdkafka::message::Message as RdMessage;
            use tokio::sync::broadcast::error::TryRecvError;

            let mut emitted = 0usize;
            loop {
                // Check cancellation
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Closed) => {
                        let _ = win.emit("kafka:load_cancelled", &serde_json::json!({}));
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Lagged(_)) => {}
                }

                // If all partitions are already done, finish
                if !parts.is_empty() && parts.iter().all(|p| done_parts_local.contains(p)) {
                    let _ = win.emit("kafka:load_done", &serde_json::json!({ "emitted": emitted }));
                    break;
                }

                match consumer.as_ref().poll(std::time::Duration::from_millis(200)) {
                    Some(Ok(m)) => {
                        let partition = m.partition();
                        let offset = m.offset();
                        let end = *ends.get(&partition).unwrap_or(&i64::MAX);

                        // If we've reached or passed the snapshot end, mark as done and skip
                        if offset >= end {
                            done_parts_local.insert(partition);
                            continue;
                        }

                        // Decode key and payload similarly to Kafka::decode
                        let key_s = m.key().map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
                        let payload_s;
                        let mut decoding_error: Option<String> = None;
                        if matches!(message_type, crate::kafka::MessageType::Protobuf) {
                            if let (Some(pd), Some(bytes)) = (proto_decoder.as_ref(), m.payload()) {
                                match pd.decode(bytes) {
                                    Ok(json) => { payload_s = json; }
                                    Err(e) => {
                                        payload_s = String::from_utf8_lossy(bytes).to_string();
                                        decoding_error = Some(format!("Protobuf decode error: {}", e));
                                    }
                                }
                            } else {
                                payload_s = m.payload().map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or_default();
                            }
                        } else {
                            let dec = crate::kafka::decoder_for(&message_type);
                            let (_k, v) = dec.decode(None, m.payload());
                            payload_s = v;
                        }

                        // Apply filters and emit if matched
                        let mut pass = true;
                        // Key filter (plain contains)
                        if let Some(kf) = key_filter.as_ref().and_then(|s| if s.is_empty() { None } else { Some(s) }) {
                            pass &= key_s.to_lowercase().contains(&kf.to_lowercase());
                        }
                        // Message filter: jq or plain contains
                        if pass {
                            if let Some(mf) = msg_filter.as_ref().and_then(|s| if s.is_empty() { None } else { Some(s) }) {
                                match filter_mode {
                                    FilterMode::Jq => {
                                        // Try parse as JSON and evaluate; if parsing fails or eval is false, drop
                                        match serde_json::from_str::<serde_json::Value>(&payload_s) {
                                            Ok(val) => match eval_jq_bool(mf, &val) {
                                                Ok(true) => { /* keep pass = true */ }
                                                _ => { pass = false; }
                                            },
                                            Err(_) => { pass = false; }
                                        }
                                    }
                                    FilterMode::Plain => {
                                        pass &= payload_s.to_lowercase().contains(&mf.to_lowercase());
                                    }
                                }
                            }
                        }

                        if pass {
                            let ts_str = match m.timestamp() {
                                rdkafka::message::Timestamp::NotAvailable => String::new(),
                                rdkafka::message::Timestamp::CreateTime(ms)
                                | rdkafka::message::Timestamp::LogAppendTime(ms) => {
                                    if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) {
                                        dt.to_rfc3339()
                                    } else { String::new() }
                                }
                            };
                            let ui = UiMessage {
                                id: format!("{}-{}", partition, offset),
                                partition,
                                key: key_s,
                                offset,
                                message: payload_s,
                                timestamp: ts_str,
                                decoding_error,
                            };
                            let _ = win.emit("kafka:message", &ui);
                            emitted += 1;
                            if emitted >= limit {
                                let _ = win.emit("kafka:load_done", &serde_json::json!({ "emitted": emitted }));
                                break;
                            }
                        }

                        // After processing, if we've emitted the last offset in the snapshot, mark partition done
                        if offset >= end - 1 {
                            done_parts_local.insert(partition);
                        }
                    }
                    Some(Err(_)) | None => {
                        // No message in this poll window; just continue to allow cancel or new data
                    }
                }
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn cancel_filtered_load(state: State<'_, AppState>) -> Result<(), String> {
    let mut sess_guard = state.load_session.lock().map_err(|e| format!("Failed to access load session: {e}"))?;
    if let Some(s) = sess_guard.take() {
        let _ = s.cancel_tx.send(());
    }
    Ok(())
}

/// Copy a selected file into an application-managed directory and return its new path.
/// kind can be one of: "truststore", "keystore", "proto" (used for namespacing), or any string.
#[tauri::command]
pub async fn import_app_file(_app: AppHandle, src_path: String, kind: Option<String>) -> Result<String, String> {
    use std::fs;
    use std::path::{Path, PathBuf};

    let src = Path::new(&src_path);
    if !src.exists() {
        return Err(format!("Source file does not exist: {}", src_path));
    }

    // Destination root: OS temp dir + rkui_uploads
    let mut dest_root: PathBuf = std::env::temp_dir();
    dest_root.push("rkui_uploads");
    let ns = kind.unwrap_or_else(|| "misc".to_string());
    dest_root.push(ns);

    // Ensure destination directory exists
    if let Err(e) = fs::create_dir_all(&dest_root) {
        return Err(format!("Failed to create upload directory: {}", e));
    }

    // Determine a unique destination filename
    let orig_name = src.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S%3f");
    let dest_name = format!("{}_{}", ts, orig_name);
    let dest_path = dest_root.join(dest_name);

    // Copy the file
    if let Err(e) = fs::copy(&src, &dest_path) {
        return Err(format!("Failed to copy file: {}", e));
    }

    Ok(dest_path.to_string_lossy().to_string())
}
