use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use protobuf::descriptor::{DescriptorProto, FileDescriptorProto, FileDescriptorSet};
use protobuf::reflect::{FileDescriptor, MessageDescriptor};

use crate::utils::{link_file_descriptors, normalize_full_name, run_protoc_and_read_descriptor_set};

#[derive(Debug, Serialize)]
pub struct ProtoMetadata {
    pub packages: Vec<String>,
    pub messages: Vec<String>,
}


fn collect_messages_from_descriptor(
    pkg: &str,
    desc: &DescriptorProto,
    out: &mut Vec<String>,
    prefix: Option<&str>,
) {
    let name = desc.name();
    if name.is_empty() {
        return;
    }
    let mut fq = String::new();
    if !pkg.is_empty() {
        fq.push_str(pkg);
        fq.push('.');
    }
    if let Some(pref) = prefix {
        if !pref.is_empty() {
            fq.push_str(pref);
            fq.push('.');
        }
    }
    fq.push_str(name);
    out.push(fq);

    // Recurse into nested types
    let new_prefix = if let Some(pref) = prefix {
        if pref.is_empty() { name.to_string() } else { format!("{}.{}", pref, name) }
    } else {
        name.to_string()
    };
    for nested in &desc.nested_type {
        collect_messages_from_descriptor(pkg, nested, out, Some(&new_prefix));
    }
}

fn extract_from_file_descriptor(fd: &FileDescriptorProto, packages: &mut HashSet<String>, messages: &mut Vec<String>) {
    let pkg = fd.package().to_string();
    if !pkg.is_empty() {
        packages.insert(pkg.clone());
    }
    for msg in &fd.message_type {
        collect_messages_from_descriptor(&pkg, msg, messages, None);
    }
}

pub struct ProtoDecoder {
    // Parsed and typechecked descriptors
    files: Vec<FileDescriptor>,
    // If provided by UI, decode using this full name
    message_full_name: Option<String>,
}

impl ProtoDecoder {
    pub fn from_proto_files(files: Vec<String>, selected_message: Option<String>) -> Result<Arc<Self>, String> {
        if files.is_empty() {
            return Err("No .proto files provided".into());
        }

        // Expand provided paths:
        // - If a directory is provided, include all *.proto inside (non-recursive).
        // - If a file is provided, include it plus all sibling *.proto files in the same directory.
        let mut uniq: std::collections::HashSet<String> = std::collections::HashSet::new();
        for f in &files {
            let p = Path::new(f);
            if p.is_dir() {
                // Canonicalize directory for stable sibling enumeration
                let dir = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
                let rd = std::fs::read_dir(&dir).map_err(|e| format!("Failed to read dir '{}': {}", dir.display(), e))?;
                for ent in rd.filter_map(|e| e.ok()) {
                    let path = ent.path();
                    if path.is_file() && path.extension().map(|e| e == "proto").unwrap_or(false) {
                        let can = std::fs::canonicalize(&path).unwrap_or(path);
                        uniq.insert(can.to_string_lossy().to_string());
                    }
                }
            } else if p.is_file() {
                // include the file itself (canonicalized for stability)
                let can = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
                uniq.insert(can.to_string_lossy().to_string());
            } else {
                return Err(format!("File or directory not found: {}", f));
            }
        }
        let mut expanded: Vec<String> = uniq.into_iter().collect();
        expanded.sort();
        if expanded.is_empty() {
            return Err("No .proto files found from provided paths".into());
        }
        for f in &expanded {
            if !Path::new(f).exists() {
                return Err(format!("File not found: {}", f));
            }
        }

        // Build descriptors using protoc-produced descriptor set and link into reflect FileDescriptor graph
        let pb_fds: FileDescriptorSet = run_protoc_and_read_descriptor_set(&expanded)?;
        let built: Vec<FileDescriptor> = link_file_descriptors(&pb_fds)?;

        // Accept the selected message from UI as-is (normalize)
        let chosen = selected_message.map(normalize_full_name);

        Ok(Arc::new(Self { files: built, message_full_name: chosen }))
    }

    pub fn decode(&self, payload: &[u8]) -> Result<String, String> {
        // Require an explicitly selected message to avoid expensive guessing and keep UI fast.
        let name = match &self.message_full_name {
            Some(n) => n,
            None => return Err("No protobuf message is selected. Select a specific message type to enable decoding.".to_string()),
        };

        // Resolve message descriptor by full name (accepts with or without leading dot)
        let resolve_msg = |name: &str| -> Result<MessageDescriptor, String> {
            let fq = if name.starts_with('.') { name.to_string() } else { format!(".{}", name) };
            self.files
                .iter()
                .find_map(|fd| fd.message_by_full_name(&fq))
                .ok_or_else(|| format!("Message type not found in descriptors: {}", fq))
        };

        // Build a list of candidate views of the payload (raw and common envelopes) without allocating copies.
        let mut views: Vec<&[u8]> = Vec::with_capacity(10);

        // 1) raw bytes
        views.push(payload);

        // Helper: parse a single unsigned varint; returns (consumed_len, value)
        let parse_varint = |bytes: &[u8]| -> Option<(usize, u64)> {
            let mut val: u64 = 0;
            let mut shift: u32 = 0;
            let mut i = 0usize;
            while i < bytes.len() && i < 10 {
                let b = bytes[i];
                val |= ((b & 0x7F) as u64) << shift;
                shift += 7;
                i += 1;
                if b & 0x80 == 0 { break; }
            }
            if i == 0 { return None; }
            // Completed only if last byte had MSB=0
            if bytes.get(i - 1).map(|b| b & 0x80 == 0).unwrap_or(false) {
                Some((i, val))
            } else {
                None
            }
        };

        // 2) Confluent Schema Registry envelope: magic 0 + 4-byte schema id, possibly followed by varint indices
        if payload.len() > 5 && payload[0] == 0 {
            // Base after header
            let base = &payload[5..];
            views.push(base);

            // Try skipping N consecutive varints (N=1..=5)
            for n in 1..=5 {
                let mut off = 0usize;
                let mut ok = true;
                for _ in 0..n {
                    if let Some((consumed, _)) = parse_varint(&base[off..]) {
                        off += consumed;
                    } else {
                        ok = false;
                        break;
                    }
                }
                if ok && off < base.len() {
                    views.push(&base[off..]);
                } else if !ok {
                    break;
                }
            }

            // Try the count + indices pattern: first varint is count C (capped), then skip C varints
            if let Some((c1, cnt)) = parse_varint(base) {
                let c = std::cmp::min(cnt as usize, 5);
                let mut off = c1;
                let mut ok = true;
                for _ in 0..c {
                    if let Some((consumed, _)) = parse_varint(&base[off..]) {
                        off += consumed;
                    } else {
                        ok = false;
                        break;
                    }
                }
                if ok && off < base.len() {
                    views.push(&base[off..]);
                }
            }
        }

        // 3) gRPC framing: 1 byte flag + 4 byte big-endian length
        if payload.len() >= 5 {
            let flag = payload[0];
            let len = u32::from_be_bytes([payload[1], payload[2], payload[3], payload[4]]) as usize;
            if (flag == 0 || flag == 1) && payload.len() >= 5 + len {
                views.push(&payload[5..5 + len]);
            }
        }

        // 4) Bare varint length prefix (non-gRPC)
        {
            let mut shift = 0u32;
            let mut len_val: usize = 0;
            let mut i = 0usize;
            while i < payload.len() && i < 10 {
                let b = payload[i];
                len_val |= ((b & 0x7F) as usize) << shift;
                shift += 7;
                i += 1;
                if b & 0x80 == 0 { break; }
            }
            if i > 0 && len_val > 0 && payload.len() >= i + len_val {
                views.push(&payload[i..i + len_val]);
            }
        }

        // Resolve descriptor once and try to parse views
        let md = resolve_msg(name)?;
        for bytes in &views {
            match md.parse_from_bytes(bytes) {
                Ok(msg) => match protobuf_json_mapping::print_to_string(&*msg) {
                    Ok(json) => {
                        // Ensure compact JSON without spaces by reserializing via serde_json
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
                            return Ok(serde_json::to_string(&val).unwrap_or(json));
                        }
                        return Ok(json);
                    }
                    Err(_e) => { /* keep trying other views */ }
                },
                Err(_e) => { /* keep trying other views */ }
            }
        }

        // 1a) Lazy repair attempt: if the payload is missing the first tag byte (common for field #1 length-delimited -> 0x0A)
        let mut repaired = Vec::with_capacity(payload.len() + 1);
        repaired.push(0x0A);
        repaired.extend_from_slice(payload);
        match md.parse_from_bytes(&repaired) {
            Ok(msg) => match protobuf_json_mapping::print_to_string(&*msg) {
                Ok(json) => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
                        return Ok(serde_json::to_string(&val).unwrap_or(json));
                    }
                    return Ok(json);
                }
                Err(e) => return Err(format!("Failed to serialize protobuf JSON: {}", e)),
            },
            Err(e) => return Err(format!("Failed to parse protobuf payload as .{} (repaired): {}", name, e)),
        }
    }
}

#[tauri::command]
pub async fn parse_proto_metadata(files: Vec<String>) -> Result<ProtoMetadata, String> {
    if files.is_empty() {
        return Err("No files provided".into());
    }

    // Expand provided paths (mirror ProtoDecoder::from_proto_files behavior)
    let mut uniq: std::collections::HashSet<String> = std::collections::HashSet::new();
    for f in &files {
        let p = Path::new(f);
        if p.is_dir() {
            // Canonicalize directory for stable sibling enumeration
            let dir = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            let rd = std::fs::read_dir(&dir).map_err(|e| format!("Failed to read dir '{}': {}", dir.display(), e))?;
            for ent in rd.filter_map(|e| e.ok()) {
                let path = ent.path();
                if path.is_file() && path.extension().map(|e| e == "proto").unwrap_or(false) {
                    let can = std::fs::canonicalize(&path).unwrap_or(path);
                    uniq.insert(can.to_string_lossy().to_string());
                }
            }
        } else if p.is_file() {
            // include the file itself (canonicalized for stability)
            let can = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            uniq.insert(can.to_string_lossy().to_string());
        } else {
            return Err(format!("File or directory not found: {}", f));
        }
    }
    let mut expanded: Vec<String> = uniq.into_iter().collect();
    expanded.sort();
    if expanded.is_empty() {
        return Err("No .proto files found from provided paths".into());
    }

    // Build descriptor set via shared utility
    let fds: FileDescriptorSet = run_protoc_and_read_descriptor_set(&expanded)?;

    let mut packages_set: HashSet<String> = HashSet::new();
    let mut messages: Vec<String> = Vec::new();

    for fd in &fds.file {
        extract_from_file_descriptor(fd, &mut packages_set, &mut messages);
    }

    messages.sort();
    messages.dedup();

    let mut packages: Vec<String> = packages_set.into_iter().collect();
    packages.sort();

    Ok(ProtoMetadata { packages, messages })
}
