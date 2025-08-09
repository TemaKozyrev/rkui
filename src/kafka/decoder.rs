use serde::{Deserialize, Serialize};

/// MessageType lists supported payload formats.
/// Keeping it here decouples decoding from the Kafka consumer logic
/// and allows adding formats without touching reader/consumer code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "json")] Json,
    #[serde(rename = "text")] Text,
    #[serde(rename = "protobuf")] Protobuf,
}

/// Trait for decoding a raw Kafka payload into a UI-presentable string.
/// In the future, this could return structured data or a richer enum.
pub trait MessageDecoder: Send + Sync {
    fn decode(&self, key: Option<&[u8]>, payload: Option<&[u8]>) -> (String, String);
}

/// Simple decoder that just UTF-8 lossy decodes key and payload.
struct TextDecoder;
impl MessageDecoder for TextDecoder {
    fn decode(&self, key: Option<&[u8]>, payload: Option<&[u8]>) -> (String, String) {
        let k = key.map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
        let v = payload.map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
        (k, v)
    }
}

/// JSON decoder for future extension. Currently identical to TextDecoder,
/// but kept separate to allow validation/pretty-printing later.
struct JsonDecoder;
impl MessageDecoder for JsonDecoder {
    fn decode(&self, key: Option<&[u8]>, payload: Option<&[u8]>) -> (String, String) {
        let k = key.map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
        let v = payload.map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
        (k, v)
    }
}

/// Protobuf decoder placeholder. Later we can plug a dynamic schema registry
/// or local descriptor set and render JSON. For now it's the same as Text.
struct ProtobufDecoder;
impl MessageDecoder for ProtobufDecoder {
    fn decode(&self, key: Option<&[u8]>, payload: Option<&[u8]>) -> (String, String) {
        let k = key.map(|k| String::from_utf8_lossy(k).to_string()).unwrap_or_default();
        let v = payload.map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default();
        (k, v)
    }
}

/// Factory for decoder instances. Light-weight and cheap to construct.
pub fn decoder_for(ty: &MessageType) -> Box<dyn MessageDecoder> {
    match ty {
        MessageType::Json => Box::new(JsonDecoder),
        MessageType::Text => Box::new(TextDecoder),
        MessageType::Protobuf => Box::new(ProtobufDecoder),
    }
}
