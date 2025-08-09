use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::collections::HashMap;

use protobuf::descriptor::{DescriptorProto, FileDescriptorProto, FileDescriptorSet};
use protobuf::Message; // for parse_from_bytes
use protobuf::reflect::{FileDescriptor, MessageDescriptor};

#[derive(Debug, Serialize)]
pub struct ProtoMetadata {
    pub packages: Vec<String>,
    pub messages: Vec<String>,
}

fn unique_parent_dirs(files: &[String]) -> Vec<PathBuf> {
    let mut set: HashSet<PathBuf> = HashSet::new();
    for f in files {
        if let Some(parent) = Path::new(f).parent() {
            set.insert(parent.to_path_buf());
        }
    }
    set.into_iter().collect()
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
    // If provided by UI, decode using this full name; otherwise try candidates sequentially
    message_full_name: Option<String>,
    candidates: Vec<String>,
}

impl ProtoDecoder {
    pub fn from_proto_files(files: Vec<String>, selected_message: Option<String>) -> Result<Arc<Self>, String> {
        if files.is_empty() {
            return Err("No .proto files provided".into());
        }
        for f in &files {
            if !Path::new(f).exists() {
                return Err(format!("File not found: {}", f));
            }
        }
        // Build descriptors using protoc-produced descriptor set and link into reflect FileDescriptor graph
        let protoc_path = protoc_bin_vendored::protoc_bin_path().map_err(|e| format!("Failed to locate protoc: {e}"))?;
        let tmp = tempfile::NamedTempFile::new().map_err(|e| format!("Failed to create temp file: {e}"))?;
        let out_path = tmp.path().to_path_buf();
        let include_dirs = unique_parent_dirs(&files);
        let mut cmd = Command::new(protoc_path);
        cmd.arg("--include_imports");
        cmd.arg(format!("--descriptor_set_out={}", out_path.display()));
        for inc in &include_dirs {
            cmd.arg("-I");
            cmd.arg(inc);
        }
        for f in &files {
            cmd.arg(f);
        }
        let output = cmd.output().map_err(|e| format!("Failed to run protoc: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("protoc failed: {}", stderr.trim()));
        }
        let bytes = std::fs::read(&out_path).map_err(|e| format!("Failed to read descriptor set: {e}"))?;
        let pb_fds: FileDescriptorSet = Message::parse_from_bytes(&bytes)
            .map_err(|e| format!("Failed to parse descriptor set (protobuf): {e}"))?;
        // Topologically link FileDescriptorProto -> reflect::FileDescriptor
        let mut remaining: Vec<FileDescriptorProto> = pb_fds.file.clone();
        let mut built: Vec<FileDescriptor> = Vec::new();
        let mut built_map: HashMap<String, usize> = HashMap::new();
        let mut progress = true;
        while !remaining.is_empty() && progress {
            progress = false;
            let mut i = 0;
            while i < remaining.len() {
                let fd = &remaining[i];
                let deps_ready = fd.dependency.iter().all(|dep| built_map.contains_key(dep));
                if deps_ready {
                    let deps_idx: Vec<usize> = fd.dependency.iter().map(|d| built_map[d]).collect();
                    let deps_vec: Vec<FileDescriptor> = deps_idx.iter().map(|&idx| built[idx].clone()).collect();
                    match FileDescriptor::new_dynamic(fd.clone(), deps_vec.as_slice()) {
                        Ok(fdesc) => {
                            built_map.insert(fd.name().to_string(), built.len());
                            built.push(fdesc);
                            remaining.remove(i);
                            progress = true;
                            continue;
                        }
                        Err(e) => {
                            return Err(format!("Failed to build FileDescriptor for {}: {}", fd.name(), e));
                        }
                    }
                }
                i += 1;
            }
        }
        if !remaining.is_empty() {
            let rest: Vec<String> = remaining.iter().map(|f| f.name().to_string()).collect();
            return Err(format!("Failed to resolve dependencies for proto files: {:?}", rest));
        }
        // Collect message names for UI compatibility
        let mut messages: Vec<String> = Vec::new();
        let mut pkgs: HashSet<String> = HashSet::new();
        for fd in &pb_fds.file {
            extract_from_file_descriptor(fd, &mut pkgs, &mut messages);
        }
        messages.sort();
        messages.dedup();

        // Accept the selected message from UI as-is (normalize), even if not present in the
        // messages list produced from this particular descriptor set. This avoids dropping a
        // valid selection when the UI parsed multiple files but the decoder was built from a
        // subset that still contains compatible types.
        let chosen = selected_message.map(|mut sel| {
            // Normalize leading dot if provided by other tools (we store without leading dot)
            if sel.starts_with('.') { sel.remove(0); }
            sel
        });

        Ok(Arc::new(Self { files: built, message_full_name: chosen, candidates: messages }))
    }

    pub fn decode(&self, payload: &[u8]) -> Result<String, String> {
        // Resolve message descriptor by full name (accepts with or without leading dot)
        let resolve_msg = |name: &str| -> Result<MessageDescriptor, String> {
            let fq = if name.starts_with('.') { name.to_string() } else { format!(".{}", name) };
            self.files
                .iter()
                .find_map(|fd| fd.message_by_full_name(&fq))
                .ok_or_else(|| format!("Message type not found in descriptors: {}", fq))
        };

        // Build a list of candidate views of the payload (raw and common envelopes)
        // Keep ordering stable and logic simple.
        let mut views: Vec<&[u8]> = Vec::with_capacity(10);

        // 1) raw bytes
        views.push(payload);

        // 1a) Repair attempt: if the payload is missing the first tag byte (common for field #1 length-delimited -> 0x0A),
        // try to preprend 0x0A and decode. This is a conservative heuristic placed after raw so it won't affect valid inputs.
        let mut owned_views: Vec<Vec<u8>> = Vec::new();
        {
            let mut repaired = Vec::with_capacity(payload.len() + 1);
            repaired.push(0x0A);
            repaired.extend_from_slice(payload);
            owned_views.push(repaired);
            let last = owned_views.last().unwrap();
            views.push(last.as_slice());
        }

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

        // Try selected message first
        let mut last_err: Option<String> = None;
        let mut try_with_name = |name: &str| -> Option<String> {
            let md = match resolve_msg(name) {
                Ok(m) => m,
                Err(e) => { last_err = Some(e); return None; }
            };
            for bytes in &views {
                match md.parse_from_bytes(bytes) {
                    Ok(msg) => match protobuf_json_mapping::print_to_string(&*msg) {
                        Ok(json) => return Some(json),
                        Err(e) => { last_err = Some(format!("Failed to serialize protobuf JSON: {}", e)); }
                    },
                    Err(e) => { last_err = Some(format!("Failed to parse protobuf payload as .{}: {}", name, e)); }
                }
            }
            None
        };

        if let Some(name) = &self.message_full_name {
            if let Some(json) = try_with_name(name) {
                return Ok(json);
            }
        }
        for name in &self.candidates {
            if let Some(json) = try_with_name(name) {
                return Ok(json);
            }
        }

        Err(last_err.unwrap_or_else(|| "Failed to decode payload with available message types".to_string()))
    }
}

#[tauri::command]
pub async fn parse_proto_metadata(files: Vec<String>) -> Result<ProtoMetadata, String> {
    if files.is_empty() {
        return Err("No files provided".into());
    }
    // Validate files exist
    for f in &files {
        if !Path::new(f).exists() {
            return Err(format!("File not found: {}", f));
        }
    }

    // Prepare protoc command using vendored binary
    let protoc_path = protoc_bin_vendored::protoc_bin_path().map_err(|e| format!("Failed to locate protoc: {e}"))?;

    let tmp = tempfile::NamedTempFile::new().map_err(|e| format!("Failed to create temp file: {e}"))?;
    let out_path = tmp.path().to_path_buf();

    let include_dirs = unique_parent_dirs(&files);

    let mut cmd = Command::new(protoc_path);
    cmd.arg("--include_imports");
    cmd.arg(format!("--descriptor_set_out={}", out_path.display()));
    for inc in &include_dirs {
        cmd.arg("-I");
        cmd.arg(inc);
    }
    for f in &files {
        cmd.arg(f);
    }

    let output = cmd.output().map_err(|e| format!("Failed to run protoc: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("protoc failed: {}", stderr.trim()));
    }

    let bytes = std::fs::read(&out_path).map_err(|e| format!("Failed to read descriptor set: {e}"))?;
    let fds: FileDescriptorSet = Message::parse_from_bytes(&bytes)
        .map_err(|e| format!("Failed to parse descriptor set: {e}"))?;

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
