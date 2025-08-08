mod app;
mod kafka;
mod kafka_adapter;

use app::AppState;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            kafka_adapter::set_kafka_config,
            kafka_adapter::get_kafka_status,
            kafka_adapter::get_topics,
            kafka_adapter::get_topic_partitions,
            kafka_adapter::apply_filters,
            kafka_adapter::consume_next_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
