pub mod kafka;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use protobuf::descriptor::{FileDescriptorProto, FileDescriptorSet};
use protobuf::reflect::FileDescriptor;

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

/// Parse .proto files using pure-Rust parser (no external protoc) and return FileDescriptorSet.
pub fn run_protoc_and_read_descriptor_set(files: &[String]) -> Result<FileDescriptorSet, String> {
    if files.is_empty() {
        return Err("No .proto files provided".into());
    }
    for f in files {
        if !Path::new(f).exists() {
            return Err(format!("File not found: {}", f));
        }
    }

    // Collect include directories from provided files so imports can be resolved
    let include_dirs = unique_parent_dirs(files);

    // Use protobuf-parse (pure Rust) to parse and typecheck the .proto files
    let mut parser = protobuf_parse::Parser::new();
    parser
        .includes(include_dirs.iter().map(|p| p.as_path()))
        .inputs(files.iter().map(|s| Path::new(s)));

    let parsed = parser
        .parse_and_typecheck()
        .map_err(|e| format!("Failed to parse .proto files: {e}"))?;

    // Build FileDescriptorSet from parsed descriptors
    let set = FileDescriptorSet {
        file: parsed.file_descriptors, // Vec<FileDescriptorProto>
        ..Default::default()
    };
    Ok(set)
}

/// Link FileDescriptorProto entries into reflect::FileDescriptor graph, resolving dependencies.
pub fn link_file_descriptors(fds: &FileDescriptorSet) -> Result<Vec<FileDescriptor>, String> {
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    let mut remaining: Vec<FileDescriptorProto> = fds.file.clone();
    let mut built: Vec<FileDescriptor> = Vec::new();

    // Maps for resolving dependencies among already built descriptors
    let mut built_full: HashMap<String, usize> = HashMap::new();
    let mut built_base: HashMap<String, usize> = HashMap::new();
    let mut base_collisions: HashSet<String> = HashSet::new();

    let mut progress = true;
    while !remaining.is_empty() && progress {
        progress = false;
        let mut i = 0;
        while i < remaining.len() {
            let fd = &remaining[i];

            // Helper to resolve a dependency path to an index of an already built descriptor
            let resolve_dep = |dep: &str| -> Option<usize> {
                if let Some(&idx) = built_full.get(dep) { return Some(idx); }
                // Try by basename if unique
                let base = Path::new(dep).file_name()?.to_string_lossy().to_string();
                if !base_collisions.contains(&base) {
                    if let Some(&idx) = built_base.get(&base) { return Some(idx); }
                }
                None
            };

            let deps_ready = fd.dependency.iter().all(|dep| resolve_dep(dep).is_some());
            if deps_ready {
                let deps_idx: Vec<usize> = fd
                    .dependency
                    .iter()
                    .map(|d| resolve_dep(d).expect("dep must be ready"))
                    .collect();
                let deps_vec: Vec<FileDescriptor> = deps_idx.iter().map(|&idx| built[idx].clone()).collect();
                match FileDescriptor::new_dynamic(fd.clone(), deps_vec.as_slice()) {
                    Ok(fdesc) => {
                        // Insert into maps
                        let full = fd.name().to_string();
                        built_full.insert(full.clone(), built.len());
                        if let Some(os) = Path::new(&full).file_name() {
                            let base = os.to_string_lossy().to_string();
                            if let Some(existing) = built_base.get(&base).copied() {
                                if existing != built.len() {
                                    // mark collision
                                    built_base.remove(&base);
                                    base_collisions.insert(base);
                                }
                            } else if !base_collisions.contains(&base) {
                                built_base.insert(base, built.len());
                            }
                        }
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
