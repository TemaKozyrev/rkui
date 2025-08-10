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

    // JSON must include fields
    assert!(json.contains("name"), "json should include 'name': {}", json);
    assert!(json.contains("id"), "json should include 'id': {}", json);
    assert!(json.contains("email"), "json should include 'email': {}", json);

    // And it must be minified (no extra spaces/newlines). We verify by
    // checking it equals serde_json's default compact serialization.
    let val: serde_json::Value = serde_json::from_str(&json).expect("decoder must output valid JSON");
    let compact = serde_json::to_string(&val).unwrap();
    assert_eq!(json, compact, "Protobuf JSON must be compact (no spaces/newlines). Got: {}", json);
}


#[test]
fn descriptor_set_includes_person_when_only_import_is_passed() {
    // Build an expanded list similar to ProtoDecoder::from_proto_files
    let p = std::fs::canonicalize("address.proto").expect("address.proto must exist");
    let mut files = std::collections::HashSet::new();
    files.insert(p.to_string_lossy().to_string());
    let dir = p.parent().expect("has parent directory");
    for ent in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let sp = ent.path();
        if sp.is_file() && sp.extension().map(|e| e == "proto").unwrap_or(false) {
            let can = std::fs::canonicalize(&sp).unwrap_or(sp);
            files.insert(can.to_string_lossy().to_string());
        }
    }
    let mut vec_files: Vec<String> = files.into_iter().collect();
    vec_files.sort();

    let fds = rkui::utils::run_protoc_and_read_descriptor_set(&vec_files).expect("protoc ok");
    let mut found = false;
    for fd in &fds.file {
        if fd.package() == "example" {
            for mt in &fd.message_type {
                if mt.name() == "Person" { found = true; break; }
            }
        }
        if found { break; }
    }
    assert!(found, "descriptor set should contain example.Person");
}

#[test]
fn decode_person_when_only_import_is_passed() {
    // Arrange: initialize decoder with only the imported file address.proto
    let proto_path = "address.proto".to_string();
    let decoder = ProtoDecoder::from_proto_files(vec![proto_path], Some("example.Person".to_string()))
        .expect("failed to init proto decoder with imported-only file");

    // Use the same generated binary; it contains example.Person
    let bytes = fs::read("proto_message.bin").expect("proto_message.bin should exist");

    // Act
    let res = decoder.decode(&bytes);

    // Assert: should still succeed due to sibling .proto expansion
    let json = res.expect("decode should succeed when only import is passed");
    let val: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(val.get("name").is_some(), "json should include 'name': {}", json);
}