use serde::{Deserialize, Serialize};

use super::decoder::MessageType;

/// UI-facing message representation. Keep it small and serializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMessage {
    pub id: String,
    pub partition: i32,
    pub key: String,
    pub offset: i64,
    pub message: String,
    pub timestamp: String,
    pub decoding_error: Option<String>,
}

/// Kafka connection and reading configuration coming from the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub broker: String,
    pub topic: String,
    // Legacy flag kept for backward compatibility with older UIs
    pub ssl_enabled: bool,
    /// Optional advanced truststore/keystore settings (mostly for SASL SSL, Java-like mode)
    #[serde(rename = "truststore_location", alias = "truststoreLocation")]
    pub truststore_location: Option<String>,
    #[serde(rename = "truststore_password", alias = "truststorePassword")]
    pub truststore_password: Option<String>,
    /// Optional selection of SSL mode when using SSL/SASL_SSL: "java_like" | "classic"
    #[serde(rename = "ssl_mode", alias = "sslMode")]
    pub ssl_mode: Option<String>,
    /// Classic SSL fields (PEM/CRT files) - optional
    #[serde(rename = "ssl_ca_root", alias = "sslCaRoot")]
    pub ssl_ca_root: Option<String>,
    #[serde(rename = "ssl_ca_sub", alias = "sslCaSub")]
    pub ssl_ca_sub: Option<String>,
    #[serde(rename = "ssl_certificate", alias = "sslCertificate")]
    pub ssl_certificate: Option<String>,
    #[serde(rename = "ssl_key", alias = "sslKey")]
    pub ssl_key: Option<String>,
    #[serde(rename = "ssl_key_password", alias = "sslKeyPassword")]
    pub ssl_key_password: Option<String>,
    /// Optional security type sent by the UI: "plaintext" | "ssl" | "sasl_plaintext" | "sasl_ssl"
    #[serde(rename = "security_type", alias = "securityType")]
    pub security_type: Option<String>,
    /// Optional SASL mechanism (e.g., PLAIN, SCRAM-SHA-256, SCRAM-SHA-512)
    #[serde(rename = "sasl_mechanism", alias = "saslMechanism")]
    pub sasl_mechanism: Option<String>,
    /// JAAS-like config string; we will parse username/password out of it
    #[serde(rename = "sasl_jaas_config", alias = "saslJaasConfig")]
    pub sasl_jaas_config: Option<String>,
    pub message_type: MessageType,
    /// "all" or a specific partition id as string
    pub partition: Option<String>,
    /// Starting offset for a specific partition (ignored when partition == "all")
    pub start_offset: Option<i64>,
    /// Start position preference: "oldest" (default) or "newest"
    #[serde(rename = "start_from", alias = "startFrom")]
    pub start_from: Option<String>,
    /// Optional path to proto schema (fallback if no cached descriptors provided)
    pub proto_schema_path: Option<String>,
    /// Optional fully qualified proto message name selected in UI
    #[serde(
        rename = "proto_message_full_name",
        alias = "protoMessageFullName",
        alias = "message_full_name",
        alias = "messageFullName"
    )]
    pub proto_message_full_name: Option<String>,
    /// Optional cache key to reuse previously loaded descriptors (preferred over proto_schema_path)
    #[serde(rename = "proto_descriptor_key", alias = "protoDescriptorKey")]
    pub proto_descriptor_key: Option<String>,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            broker: "localhost:9092".into(),
            topic: "".into(),
            ssl_enabled: false,
            truststore_location: None,
            truststore_password: None,
            ssl_mode: None,
            ssl_ca_root: None,
            ssl_ca_sub: None,
            ssl_certificate: None,
            ssl_key: None,
            ssl_key_password: None,
            security_type: None,
            sasl_mechanism: None,
            sasl_jaas_config: None,
            message_type: MessageType::Json,
            partition: None,
            start_offset: None,
            start_from: Some("oldest".into()),
            proto_schema_path: None,
            proto_message_full_name: None,
            proto_descriptor_key: None, 
        }
    }
}
