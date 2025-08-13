use rdkafka::config::ClientConfig;
use rdkafka::consumer::BaseConsumer;

use crate::utils::kafka::configure_security;
use super::types::KafkaConfig;

/// Build an rdkafka BaseConsumer configured according to KafkaConfig.
pub(crate) fn create_consumer(config: &KafkaConfig) -> anyhow::Result<BaseConsumer> {
    let mut cc = ClientConfig::new();
    cc.set("bootstrap.servers", &config.broker);
    // A default group id; for UI reading anything is fine. Could be made configurable later.
    cc.set("group.id", "rkui-consumer");

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

    // Полная настройка безопасности (PLAINTEXT/SSL/SASL*) вынесена в utils
    configure_security(&mut cc, config)?;

    let consumer: BaseConsumer = cc.create()?;
    Ok(consumer)
}
