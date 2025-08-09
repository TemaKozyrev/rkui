use rkui::kafka::KafkaConfig;
use rkui::kafka::MessageType;

#[test]
fn default_config_is_sane() {
    let cfg = KafkaConfig::default();
    assert_eq!(cfg.broker, "localhost:9092");
    assert_eq!(cfg.topic, "");
    assert!(!cfg.ssl_enabled);
    assert!(matches!(cfg.message_type, MessageType::Json));
    assert!(cfg.partition.is_none());
    assert!(cfg.start_offset.is_none());
}
