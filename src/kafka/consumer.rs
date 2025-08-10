use rdkafka::config::ClientConfig;
use rdkafka::consumer::BaseConsumer;

use super::security::parse_username_password_from_jaas;
use super::types::KafkaConfig;

/// Build an rdkafka BaseConsumer configured according to KafkaConfig.
pub(crate) fn create_consumer(config: &KafkaConfig) -> anyhow::Result<BaseConsumer> {
    let mut cc = ClientConfig::new();
    cc.set("bootstrap.servers", &config.broker);
    // A default group id; for UI reading anything is fine. Could be made configurable later.
    cc.set("group.id", "rkui-consumer");
    cc.set("enable.partition.eof", "false");
    cc.set("enable.auto.commit", "true");
    cc.set("auto.offset.reset", "earliest");

    // Determine effective security type with backward compatibility
    let sec_type = config
        .security_type
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| if config.ssl_enabled { "ssl".into() } else { "plaintext".into() });

    match sec_type.as_str() {
        "ssl" => {
            cc.set("security.protocol", "ssl");
            if let Some(path) = &config.ssl_ca_path {
                cc.set("ssl.ca.location", path);
            }
            if let Some(path) = &config.ssl_cert_path {
                cc.set("ssl.certificate.location", path);
            }
            if let Some(path) = &config.ssl_key_path {
                cc.set("ssl.key.location", path);
            }
        }
        "sasl_plaintext" => {
            cc.set("security.protocol", "sasl_plaintext");
            if let Some(mech) = &config.sasl_mechanism {
                cc.set("sasl.mechanism", mech);
            }
            // Prefer explicit username/password parsed from JAAS config string
            if let Some(jaas) = &config.sasl_jaas_config {
                if let Some((user, pass)) = parse_username_password_from_jaas(jaas) {
                    cc.set("sasl.username", &user);
                    cc.set("sasl.password", &pass);
                }
            }
        }
        _ => {
            // plaintext (default): no extra settings
        }
    }

    let consumer: BaseConsumer = cc.create()?;
    Ok(consumer)
}
