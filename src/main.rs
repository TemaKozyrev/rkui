mod app;
mod kafka;
mod kafka_adapter;
mod proto_decoder;
mod utils;

use app::AppState;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            kafka_adapter::set_kafka_config,
            kafka_adapter::get_kafka_status,
            kafka_adapter::get_topics,
            kafka_adapter::get_topic_partitions,
            kafka_adapter::apply_filters,
            kafka_adapter::consume_next_messages,
            kafka_adapter::start_filtered_load,
            kafka_adapter::cancel_filtered_load,
            proto_decoder::parse_proto_metadata,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
