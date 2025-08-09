use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use protobuf::descriptor::{FileDescriptorProto, FileDescriptorSet};
use protobuf::reflect::FileDescriptor;
use protobuf::Message;

/// Collect unique parent directories from a list of file paths.
pub fn unique_parent_dirs(files: &[String]) -> Vec<PathBuf> {
    let mut set: HashSet<PathBuf> = HashSet::new();
    for f in files {
        if let Some(parent) = Path::new(f).parent() {
            set.insert(parent.to_path_buf());
        }
    }
    set.into_iter().collect()
}

/// Run vendored protoc on provided .proto files and return parsed FileDescriptorSet.
pub fn run_protoc_and_read_descriptor_set(files: &[String]) -> Result<FileDescriptorSet, String> {
    if files.is_empty() {
        return Err("No .proto files provided".into());
    }
    for f in files {
        if !Path::new(f).exists() {
            return Err(format!("File not found: {}", f));
        }
    }

    let protoc_path = protoc_bin_vendored::protoc_bin_path()
        .map_err(|e| format!("Failed to locate protoc: {e}"))?;
    let tmp = tempfile::NamedTempFile::new()
        .map_err(|e| format!("Failed to create temp file: {e}"))?;
    let out_path = tmp.path().to_path_buf();

    let include_dirs = unique_parent_dirs(files);

    let mut cmd = Command::new(protoc_path);
    cmd.arg("--include_imports");
    cmd.arg(format!("--descriptor_set_out={}", out_path.display()))
        ;
    for inc in &include_dirs {
        cmd.arg("-I");
        cmd.arg(inc);
    }
    for f in files {
        cmd.arg(f);
    }

    let output = cmd.output().map_err(|e| format!("Failed to run protoc: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("protoc failed: {}", stderr.trim()));
    }

    let bytes = std::fs::read(&out_path)
        .map_err(|e| format!("Failed to read descriptor set: {e}"))?;
    let fds: FileDescriptorSet = Message::parse_from_bytes(&bytes)
        .map_err(|e| format!("Failed to parse descriptor set (protobuf): {e}"))?;
    Ok(fds)
}

/// Link FileDescriptorProto entries into reflect::FileDescriptor graph, resolving dependencies.
pub fn link_file_descriptors(fds: &FileDescriptorSet) -> Result<Vec<FileDescriptor>, String> {
    let mut remaining: Vec<FileDescriptorProto> = fds.file.clone();
    let mut built: Vec<FileDescriptor> = Vec::new();
    let mut built_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
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
                        return Err(format!(
                            "Failed to build FileDescriptor for {}: {}",
                            fd.name(), e
                        ));
                    }
                }
            }
            i += 1;
        }
    }
    if !remaining.is_empty() {
        let rest: Vec<String> = remaining.iter().map(|f| f.name().to_string()).collect();
        return Err(format!(
            "Failed to resolve dependencies for proto files: {:?}",
            rest
        ));
    }
    Ok(built)
}

/// Normalize a protobuf full name to not have a leading dot.
pub fn normalize_full_name(mut name: String) -> String {
    if name.starts_with('.') { name.remove(0); }
    name
}
