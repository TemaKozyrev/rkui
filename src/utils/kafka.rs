use rdkafka::config::ClientConfig;
use std::io::Write;

use crate::kafka::security::{jks_truststore_to_pem, parse_username_password_from_jaas, detect_keystore_kind, KeyStoreKind, pkcs12_to_pem};
use crate::kafka::types::KafkaConfig;

/// Apply full security configuration (security.protocol + SSL/SASL specifics) based on KafkaConfig.
/// Handles backward compatibility with legacy ssl_enabled flag and optional security_type field.
pub fn configure_security(cc: &mut ClientConfig, config: &KafkaConfig) -> anyhow::Result<()> {
    // Determine effective security type with backward compatibility
    let sec_type = config
        .security_type
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| if config.ssl_enabled { "ssl".into() } else { "plaintext".into() });

    match sec_type.as_str() {
        "ssl" => {
            cc.set("security.protocol", "ssl");
            configure_ssl(cc, config)?;
        }
        "sasl_plaintext" => {
            cc.set("security.protocol", "sasl_plaintext");
            configure_sasl(cc, config);
        }
        "sasl_ssl" => {
            cc.set("security.protocol", "sasl_ssl");
            configure_ssl(cc, config)?;
            configure_sasl(cc, config);
        }
        _ => {
            // plaintext (default): no extra settings
        }
    }
    Ok(())
}

/// Helper: decide whether a path is a valid CA file/dir for OpenSSL (PEM or directory)
pub fn is_likely_ca_path(p: &str) -> bool {
    use std::fs;
    use std::path::Path;
    let path = Path::new(p);
    if let Ok(meta) = fs::metadata(path) {
        if meta.is_dir() { return true; }
    }
    let lower = p.to_ascii_lowercase();
    lower.ends_with(".pem") || lower.ends_with(".crt") || lower.ends_with(".cer") || lower.ends_with(".bundle")
}

/// Configure SSL-related options on the given ClientConfig based on KafkaConfig (truststore/keystore handling).
/// This does not set the security.protocol itself; callers should set it to `ssl` or `sasl_ssl` beforehand.
pub fn configure_ssl(cc: &mut ClientConfig, config: &KafkaConfig) -> anyhow::Result<()> {
    // Detect classic SSL mode either explicitly via ssl_mode or implicitly if classic fields are supplied
    let ssl_mode = config.ssl_mode.as_deref().map(|s| s.to_ascii_lowercase());
    let has_classic_fields = config.ssl_ca_root.is_some()
        || config.ssl_ca_sub.is_some()
        || config.ssl_certificate.is_some()
        || config.ssl_key.is_some()
        || config.ssl_key_password.is_some();

    // Determine effective mode using match on ssl_mode
    let effective_mode = match ssl_mode.as_deref() {
        Some("classic") => "classic",
        Some("java_like") => "java_like",
        Some(_) => if has_classic_fields { "classic" } else { "java_like" },
        None => if has_classic_fields { "classic" } else { "java_like" },
    };

    match effective_mode {
        "classic" => {
            // Handle Classic SSL: CA root/sub + client cert/key
            // 1) CA location
            if config.ssl_ca_root.is_some() || config.ssl_ca_sub.is_some() {
                let ca_root = config.ssl_ca_root.as_deref();
                let ca_sub = config.ssl_ca_sub.as_deref();
                // If both provided, combine into a temporary PEM bundle; else use the one provided
                let ca_location = if let (Some(root), Some(sub)) = (ca_root, ca_sub) {
                    // Combine files into one PEM
                    let mut tmp = tempfile::Builder::new().prefix("rkui-ca-bundle-").suffix(".pem").tempfile()?;
                    if std::path::Path::new(root).exists() {
                        let data = std::fs::read(root)?; tmp.write_all(&data)?;
                        if !data.ends_with(b"\n") { tmp.write_all(b"\n")?; }
                    } else {
                        return Err(anyhow::anyhow!("CA Root file not found: {}", root));
                    }
                    if std::path::Path::new(sub).exists() {
                        let data = std::fs::read(sub)?; tmp.write_all(&data)?;
                        if !data.ends_with(b"\n") { tmp.write_all(b"\n")?; }
                    } else {
                        return Err(anyhow::anyhow!("CA Sub file not found: {}", sub));
                    }
                    let path = tmp.into_temp_path();
                    let path_str = path.to_string_lossy().to_string();
                    std::mem::forget(path);
                    path_str
                } else if let Some(root) = ca_root { root.to_string() } else if let Some(sub) = ca_sub { sub.to_string() } else { unreachable!() };
                cc.set("ssl.ca.location", ca_location);
            } else if let Some(path) = config.truststore_location.as_ref() {
                // Fallback to truststore if provided (for convenience)
                match detect_keystore_kind(path) {
                    KeyStoreKind::PemOrDir => { cc.set("ssl.ca.location", path); }
                    KeyStoreKind::JksOrJceks => {
                        match jks_truststore_to_pem(path, config.truststore_password.as_deref()) {
                            Ok(pem_path) => { cc.set("ssl.ca.location", &pem_path); }
                            Err(e) => { return Err(anyhow::anyhow!("Failed to convert JKS/JCEKS truststore to PEM: {}", e)); }
                        }
                    }
                    KeyStoreKind::Pkcs12 => {
                        match pkcs12_to_pem(path, config.truststore_password.as_deref()) {
                            Ok(pem_path) => { cc.set("ssl.ca.location", &pem_path); }
                            Err(e) => { return Err(anyhow::anyhow!("Failed to convert PKCS#12 truststore to PEM ({}). Provide a valid password or a PEM bundle.", e)); }
                        }
                    }
                    KeyStoreKind::Unknown => {
                        if is_likely_ca_path(path) { cc.set("ssl.ca.location", path); }
                        else { eprintln!("[rkui] Provided Truststore Location '{}' is of unknown format; skipping ssl.ca.location.", path); }
                    }
                }
            }

            // 2) Client identity (optional)
            if let Some(cert) = config.ssl_certificate.as_ref() {
                cc.set("ssl.certificate.location", cert);
            }
            if let Some(key) = config.ssl_key.as_ref() {
                cc.set("ssl.key.location", key);
            }
            if let Some(pw) = config.ssl_key_password.as_ref() {
                if !pw.is_empty() { cc.set("ssl.key.password", pw); }
            }
        }
        _ => {
            // Java-like mode (default / backward compatible): Truststore / CA only
            if let Some(path) = config.truststore_location.as_ref() {
                match detect_keystore_kind(path) {
                    KeyStoreKind::PemOrDir => {
                        cc.set("ssl.ca.location", path);
                    }
                    KeyStoreKind::JksOrJceks => {
                        match jks_truststore_to_pem(path, config.truststore_password.as_deref()) {
                            Ok(pem_path) => { cc.set("ssl.ca.location", &pem_path); }
                            Err(e) => { return Err(anyhow::anyhow!("Failed to convert JKS/JCEKS truststore to PEM: {}", e)); }
                        }
                    }
                    KeyStoreKind::Pkcs12 => {
                        // Convert PKCS#12 to PEM bundle for OpenSSL CA
                        match pkcs12_to_pem(path, config.truststore_password.as_deref()) {
                            Ok(pem_path) => { cc.set("ssl.ca.location", &pem_path); }
                            Err(e) => {
                                return Err(anyhow::anyhow!(
                                    "Failed to convert PKCS#12 truststore to PEM ({}). Provide a valid password or a PEM bundle.", e
                                ));
                            }
                        }
                    }
                    KeyStoreKind::Unknown => {
                        if is_likely_ca_path(path) {
                            cc.set("ssl.ca.location", path);
                        } else {
                            eprintln!("[rkui] Provided Truststore Location '{}' is of unknown format; skipping ssl.ca.location.", path);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Configure SASL-related options (mechanism and credentials parsed from JAAS string if provided).
pub fn configure_sasl(cc: &mut ClientConfig, config: &KafkaConfig) {
    // Use provided mechanism or default to SCRAM-SHA-512
    let mech = config
        .sasl_mechanism
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("SCRAM-SHA-512");
    cc.set("sasl.mechanism", mech);

    // Prefer explicit username/password parsed from JAAS config string
    if let Some(jaas) = &config.sasl_jaas_config {
        if let Some((user, pass)) = parse_username_password_from_jaas(jaas) {
            cc.set("sasl.username", &user);
            cc.set("sasl.password", &pass);
        }
    }
}
