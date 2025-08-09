use rkui::kafka::decoder_for;
use rkui::kafka::MessageType;

#[test]
fn text_decoder_roundtrip_utf8_lossy() {
    let dec = decoder_for(&MessageType::Text);
    let (k, v) = dec.decode(Some(b"key"), Some(b"value"));
    assert_eq!(k, "key");
    assert_eq!(v, "value");
}

#[test]
fn json_decoder_behaves_like_text_for_now() {
    let dec = decoder_for(&MessageType::Json);
    let (k, v) = dec.decode(None, Some(&[0xF0, 0x9F, 0x92, 0xA9])); // valid UTF-8
    assert_eq!(k, "");
    assert_eq!(v, "💩");
}

#[test]
fn protobuf_decoder_placeholder_works() {
    let dec = decoder_for(&MessageType::Protobuf);
    let (_k, v) = dec.decode(None, Some(&[0xFF, 0xFF, 0xFF])); // invalid UTF-8 -> lossy
    assert!(v.len() >= 1); // not empty
}
