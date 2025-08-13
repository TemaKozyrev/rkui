/// Utilities for parsing security-related configs.
/// Non-business logic isolated from the Kafka service.

use std::io::Write;
use std::path::Path;
use base64::Engine;

// Prefer minijks for JKS/JCEKS truststore parsing when available
pub(crate) fn jks_truststore_to_pem_via_minijks(jks_path: &str, storepass: Option<&str>) -> anyhow::Result<String> {
    if !Path::new(jks_path).exists() {
        return Err(anyhow::anyhow!("Truststore file not found: {}", jks_path));
    }
    // Note: This code is only compiled when feature `with-minijks` is enabled.
    // The exact API of `minijks` may differ; adjust as needed for your environment.
    let data = std::fs::read(jks_path)?;
    let password = storepass.unwrap_or("");
    // Use minijks to parse the JKS/JCEKS and extract trusted certs
    use minijks::{Store, Options};
    let store = Store::parse(&data, Some(Options { password: password.to_string(), ..Default::default() }))
        .map_err(|e| anyhow::anyhow!("minijks parse failed: {}", e))?;
    let mut certs: Vec<Vec<u8>> = Vec::new();
    for c in &store.certs {
        // Use raw DER from minijks
        let der = c.certificate.raw.clone();
        certs.push(der);
    }
    if certs.is_empty() {
        return Err(anyhow::anyhow!("No certificates found in truststore (minijks)"));
    }
    let mut tmp = tempfile::Builder::new().prefix("rkui-ca-").suffix(".pem").tempfile()?;
    for der in certs {
        let b64 = base64::engine::general_purpose::STANDARD.encode(der);
        tmp.write_all(b"-----BEGIN CERTIFICATE-----\n")?;
        for chunk in b64.as_bytes().chunks(64) {
            tmp.write_all(chunk)?;
            tmp.write_all(b"\n")?;
        }
        tmp.write_all(b"-----END CERTIFICATE-----\n")?;
    }
    let path = tmp.into_temp_path();
    let path_str = path.to_string_lossy().to_string();
    std::mem::forget(path);
    Ok(path_str)
}

/// Convert a JKS truststore into a PEM bundle using a native Rust parser (no Java required).
/// Returns a path to a temporary PEM file suitable for `ssl.ca.location`.
pub(crate) fn jks_truststore_to_pem_native(jks_path: &str, _storepass: Option<&str>) -> anyhow::Result<String> {
    if !Path::new(jks_path).exists() {
        return Err(anyhow::anyhow!("Truststore file not found: {}", jks_path));
    }
    let bytes = std::fs::read(jks_path)?;
    let certs = parse_jks_trusted_certs(&bytes)?;
    if certs.is_empty() {
        return Err(anyhow::anyhow!("No certificates found in truststore"));
    }
    let mut tmp = tempfile::Builder::new().prefix("rkui-ca-").suffix(".pem").tempfile()?;
    for der in certs {
        let b64 = base64::engine::general_purpose::STANDARD.encode(der);
        tmp.write_all(b"-----BEGIN CERTIFICATE-----\n")?;
        for chunk in b64.as_bytes().chunks(64) {
            tmp.write_all(chunk)?;
            tmp.write_all(b"\n")?;
        }
        tmp.write_all(b"-----END CERTIFICATE-----\n")?;
    }
    let path = tmp.into_temp_path();
    let path_str = path.to_string_lossy().to_string();
    std::mem::forget(path);
    Ok(path_str)
}

/// Unified helper: try minijks first, then fallback to native parser
pub(crate) fn jks_truststore_to_pem(jks_path: &str, storepass: Option<&str>) -> anyhow::Result<String> {
    // First attempt using minijks
    match jks_truststore_to_pem_via_minijks(jks_path, storepass) {
        Ok(p) => return Ok(p),
        Err(e1) => {
            eprintln!("[rkui] minijks parse failed: {}. Falling back to native JKS parser...", e1);
        }
    }
    // Fallback to native minimal parser
    jks_truststore_to_pem_native(jks_path, storepass)
}

/// Minimal JKS reader: extracts DER certificates from trusted cert entries (type = 1).
/// It does not validate the keystore SHA-1 integrity checksum and ignores private key entries.
fn parse_jks_trusted_certs(data: &[u8]) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut rd = Cursor::new(data);

    fn read_u32(rd: &mut Cursor<&[u8]>) -> anyhow::Result<u32> {
        use std::io::Read;
        let mut buf = [0u8; 4];
        rd.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }
    fn read_u64(rd: &mut Cursor<&[u8]>) -> anyhow::Result<u64> {
        use std::io::Read;
        let mut buf = [0u8; 8];
        rd.read_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
    fn read_java_utf_skip(rd: &mut Cursor<&[u8]>) -> anyhow::Result<()> {
        use std::io::Read;
        let mut len_b = [0u8; 2];
        rd.read_exact(&mut len_b)?;
        let len = u16::from_be_bytes(len_b) as usize;
        // Bounds check before skipping bytes
        let pos = rd.position() as usize;
        let total = rd.get_ref().len();
        if pos + len > total {
            return Err(anyhow::anyhow!("Malformed JKS: UTF length {} exceeds remaining bytes {}", len, total.saturating_sub(pos)));
        }
        // Advance by reading exactly len bytes into a small fixed buffer in chunks to avoid huge allocations
        const CHUNK: usize = 4096;
        let mut remaining = len;
        let mut buf = [0u8; CHUNK];
        while remaining > 0 {
            let to_read = remaining.min(CHUNK);
            rd.read_exact(&mut buf[..to_read])?;
            remaining -= to_read;
        }
        Ok(())
    }
    fn read_attributes_if_any(version: u32, rd: &mut Cursor<&[u8]>) -> anyhow::Result<()> {
        // In JKS v2, entries may be followed by attributes: count (u32), then key/value UTF strings
        if version >= 2 {
            let attr_count = read_u32(rd)? as usize;
            for _ in 0..attr_count {
                read_java_utf_skip(rd)?; // key
                read_java_utf_skip(rd)?; // value
            }
        }
        Ok(())
    }

    use std::io::{Cursor, Read};
    let magic = read_u32(&mut rd)?;
    if magic != 0xFEED_FEED {
        return Err(anyhow::anyhow!("Not a JKS file (bad magic)"));
    }
    let version = read_u32(&mut rd)?; // 1 or 2
    let count = read_u32(&mut rd)? as usize;

    let mut certs = Vec::new();
    for _ in 0..count {
        let tag = read_u32(&mut rd)?; // 1 = trusted cert, 2 = private key
        // alias
        read_java_utf_skip(&mut rd)?;
        // timestamp
        let _ts = read_u64(&mut rd)?;
        match tag {
            1 => {
                // cert type and bytes
                read_java_utf_skip(&mut rd)?; // type (e.g., "X.509")
                let len = read_u32(&mut rd)? as usize;
                // Bounds check
                let pos = rd.position() as usize;
                let total = rd.get_ref().len();
                if pos + len > total {
                    return Err(anyhow::anyhow!("Malformed JKS: certificate length {} exceeds remaining bytes {}", len, total.saturating_sub(pos)));
                }
                let mut buf = vec![0u8; len];
                rd.read_exact(&mut buf)?;
                certs.push(buf);
                // optional attributes (v2)
                read_attributes_if_any(version, &mut rd)?;
            }
            2 => {
                // private key entry: skip
                let key_len = read_u32(&mut rd)? as usize;
                // Bounds check
                let posk = rd.position() as usize;
                let totalk = rd.get_ref().len();
                if posk + key_len > totalk {
                    return Err(anyhow::anyhow!("Malformed JKS: private key length {} exceeds remaining bytes {}", key_len, totalk.saturating_sub(posk)));
                }
                let mut skip = vec![0u8; key_len];
                rd.read_exact(&mut skip)?;
                let chain_len = read_u32(&mut rd)? as usize;
                for _ in 0..chain_len {
                    read_java_utf_skip(&mut rd)?; // cert type
                    let clen = read_u32(&mut rd)? as usize;
                    let posc = rd.position() as usize;
                    let totalc = rd.get_ref().len();
                    if posc + clen > totalc {
                        return Err(anyhow::anyhow!("Malformed JKS: chain certificate length {} exceeds remaining bytes {}", clen, totalc.saturating_sub(posc)));
                    }
                    let mut s = vec![0u8; clen];
                    rd.read_exact(&mut s)?;
                }
                // optional attributes (v2)
                read_attributes_if_any(version, &mut rd)?;
            }
            _ => return Err(anyhow::anyhow!("Unsupported JKS entry tag: {}", tag)),
        }
    }
    // trailing 20-byte SHA-1 checksum is ignored
    Ok(certs)
}

/// Convert a JKS keystore into a PKCS#12 file (not supported natively).
/// Rationale: native .jks private key decryption is not implemented. Prefer providing a .p12/.pfx.
pub(crate) fn jks_keystore_to_pkcs12_via_keytool(_jks_path: &str, _storepass: Option<&str>) -> anyhow::Result<String> {
    Err(anyhow::anyhow!("Native .jks keystore conversion is not yet supported. Please provide a PKCS#12 (.p12/.pfx) keystore or omit keystore."))
}

/// Try to extract username and password from a JAAS-like config string.
/// Accepts common variants like:
///   username="user" password="pass";
///   username = 'user' password = 'pass'
///   username=user password=pass
/// Keys are matched case-insensitively, '=' may be surrounded by whitespace, values can be quoted or unquoted.
pub(crate) fn parse_username_password_from_jaas(s: &str) -> Option<(String, String)> {
    fn extract_case_insensitive(field: &str, s: &str) -> Option<String> {
        let fl = field.to_ascii_lowercase();
        let bytes = s.as_bytes();
        let sb = s;
        let sl = s.to_ascii_lowercase();
        let bl = sl.as_bytes();
        let fb = fl.as_bytes();
        let mut i = 0usize;
        while i + fb.len() <= bl.len() {
            if &bl[i..i + fb.len()] == fb {
                // Ensure word boundary before/after (not strictly necessary but avoids matching in values)
                let before_ok = i == 0 || !bl[i - 1].is_ascii_alphanumeric();
                let after_idx = i + fb.len();
                let after_ok = after_idx >= bl.len() || !bl[after_idx].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    // Move cursor to after field name
                    let mut j = after_idx;
                    // Skip whitespace
                    while j < bl.len() && bl[j].is_ascii_whitespace() { j += 1; }
                    // Expect '=' possibly after whitespace
                    if j < bl.len() && bl[j] == b'=' {
                        j += 1;
                        // Skip whitespace after '='
                        while j < bl.len() && bl[j].is_ascii_whitespace() { j += 1; }
                        if j >= bl.len() { return None; }
                        // Quoted or unquoted?
                        let quote = sb.as_bytes()[j] as char;
                        if quote == '"' || quote == '\'' {
                            // Find matching quote
                            j += 1; // skip opening quote
                            let start = j;
                            while j < bytes.len() {
                                if sb.as_bytes()[j] as char == quote { break; }
                                j += 1;
                            }
                            if j <= bytes.len() {
                                return Some(sb[start..j].to_string());
                            }
                            return None;
                        } else {
                            // Unquoted: read until whitespace or semicolon
                            let start = j;
                            while j < bytes.len() {
                                let ch = sb.as_bytes()[j] as char;
                                if ch.is_whitespace() || ch == ';' { break; }
                                j += 1;
                            }
                            if j > start { return Some(sb[start..j].to_string()); }
                            return None;
                        }
                    }
                }
            }
            i += 1;
        }
        None
    }
    let user = extract_case_insensitive("username", s)?;
    let pass = extract_case_insensitive("password", s)?;
    Some((user, pass))
}
