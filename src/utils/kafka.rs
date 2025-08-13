use rdkafka::config::ClientConfig;

use crate::kafka::security::{jks_keystore_to_pkcs12_via_keytool, jks_truststore_to_pem, parse_username_password_from_jaas};
use crate::kafka::types::KafkaConfig;

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
    // Truststore / CA
    if let Some(path) = config.truststore_location.as_ref() {
        let lower = path.to_ascii_lowercase();
        if is_likely_ca_path(path) {
            cc.set("ssl.ca.location", path);
        } else if lower.ends_with(".jks") || lower.ends_with(".jceks") {
            match jks_truststore_to_pem(path, config.truststore_password.as_deref()) {
                Ok(pem_path) => { cc.set("ssl.ca.location", &pem_path); }
                Err(e) => { return Err(anyhow::anyhow!("Failed to convert JKS truststore to PEM: {}", e)); }
            }
        } else {
            eprintln!("[rkui] Provided Truststore Location '{}' does not look like a PEM CA file or directory; skipping ssl.ca.location.", path);
        }
    }

    // If user mistakenly put a .p12/.pfx into Truststore Location and didn't specify keystore, use it as keystore.
    if config.keystore_location.is_none() {
        if let Some(ts) = config.truststore_location.as_ref() {
            let l = ts.to_ascii_lowercase();
            if l.ends_with(".p12") || l.ends_with(".pfx") {
                cc.set("ssl.keystore.location", ts);
                if let Some(pass) = &config.truststore_password {
                    cc.set("ssl.keystore.password", pass);
                }
            }
        }
    }

    if let Some(path) = &config.keystore_location {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".jks") || lower.ends_with(".jceks") {
            let pass = config.keystore_password.as_deref().or(config.truststore_password.as_deref());
            match jks_keystore_to_pkcs12_via_keytool(path, pass) {
                Ok(p12_path) => {
                    cc.set("ssl.keystore.location", &p12_path);
                    if let Some(p) = pass { cc.set("ssl.keystore.password", p); }
                }
                Err(e) => { return Err(anyhow::anyhow!("Failed to convert JKS keystore to PKCS#12: {}", e)); }
            }
        } else {
            cc.set("ssl.keystore.location", path);
        }
    }

    if let Some(pass) = &config.keystore_password {
        cc.set("ssl.keystore.password", pass);
    }
    if let Some(pass) = &config.keystore_key_password {
        cc.set("ssl.key.password", pass);
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
