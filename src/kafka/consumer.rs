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
    // cc.set("auto.offset.reset", "earliest");

    // Оптимизации для быстрого переназначения партиций
    cc.set("socket.timeout.ms", "10000");             // Уменьшаем таймаут сокета
    cc.set("session.timeout.ms", "10000");            // Уменьшаем таймаут сессии
    cc.set("metadata.max.age.ms", "10000");          // Уменьшаем время жизни метаданных
    cc.set("connections.max.idle.ms", "30000");       // Держим соединения дольше

    // Оптимизации для быстрого получения данных
    cc.set("fetch.wait.max.ms", "100");              // Уменьшаем время ожидания фетча
    cc.set("fetch.min.bytes", "1");                  // Минимальный размер данных для фетча
    cc.set("fetch.max.bytes", "52428800");           // Увеличиваем максимальный размер фетча (50MB)

    // Оптимизации для управления офсетами
    cc.set("enable.auto.offset.store", "false");     // Отключаем автоматическое сохранение офсетов
    cc.set("auto.offset.reset", "earliest");
    cc.set("enable.partition.eof", "false");
    cc.set("enable.auto.commit", "false");           // Отключаем автокоммит

    // Оптимизации производительности
    cc.set("queued.min.messages", "1000");          // Буферизация сообщений
    cc.set("queued.max.messages.kbytes", "51200");  // Максимальный размер буфера (50MB)

    // Оптимизации для работы с брокером
    cc.set("reconnect.backoff.ms", "100");          // Уменьшаем время между попытками реконнекта
    cc.set("reconnect.backoff.max.ms", "10000");    // Максимальное время между попытками
    cc.set("allow.auto.create.topics", "false");    // Отключаем автосоздание топиков


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
            // Use provided mechanism or default to SCRAM-SHA-512
            let mech = config
                .sasl_mechanism
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("SCRAM-SHA-512");
            cc.set("sasl.mechanism", mech);
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
