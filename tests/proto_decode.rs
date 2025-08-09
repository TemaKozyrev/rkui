use std::fs;
use rkui::proto_decoder::ProtoDecoder;

#[test]
fn decode_example_person_raw_or_length_prefixed() {
    // Arrange: build decoder from example.proto and decode proto_message.bin
    let proto_path = "example.proto".to_string();
    let decoder = ProtoDecoder::from_proto_files(vec![proto_path], Some("example.Person".to_string()))
        .expect("failed to init proto decoder");

    let bytes = fs::read("proto_message.bin").expect("proto_message.bin should exist");

    // Act
    let res = decoder.decode(&bytes);

    // Assert: should succeed and produce JSON with keys name/id/email
    let json = res.expect("decode should succeed");
    assert!(json.contains("name"), "json should include 'name': {}", json);
    assert!(json.contains("id"), "json should include 'id': {}", json);
    assert!(json.contains("email"), "json should include 'email': {}", json);
}
