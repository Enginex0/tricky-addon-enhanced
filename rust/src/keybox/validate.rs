use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::Serialize;
use tracing::warn;
use x509_parser::prelude::*;

const GOOGLE_STATUS_URL: &str = "https://android.googleapis.com/attestation/status";
const REVOCATION_TIMEOUT: Duration = Duration::from_secs(12);
const REVOCATION_CACHE_TTL: Duration = Duration::from_secs(60);
const MAX_KEYBOX_BYTES: usize = 1_048_576;
const MAX_PEM_BODY_BYTES: usize = 65_536;
const PRE_NTP_EPOCH_SECS: u64 = 1_577_836_800;

const ROOT_GOOGLE_PEM: &[u8] = include_bytes!("roots/google.pem");
const ROOT_AOSP_EC_PEM: &[u8] = include_bytes!("roots/aosp_ec.pem");
const ROOT_AOSP_RSA_PEM: &[u8] = include_bytes!("roots/aosp_rsa.pem");
const ROOT_KNOX_PEM: &[u8] = include_bytes!("roots/knox.pem");
const EMBEDDED_REVOCATION: &[u8] = include_bytes!("roots/status.json");

const OID_RSA_SHA1: &str = "1.2.840.113549.1.1.5";
const OID_RSA_SHA256: &str = "1.2.840.113549.1.1.11";
const OID_RSA_SHA384: &str = "1.2.840.113549.1.1.12";
const OID_RSA_SHA512: &str = "1.2.840.113549.1.1.13";
const OID_ECDSA_SHA1: &str = "1.2.840.10045.4.1";
const OID_ECDSA_SHA256: &str = "1.2.840.10045.4.3.2";
const OID_ECDSA_SHA384: &str = "1.2.840.10045.4.3.3";
const OID_ECDSA_SHA512: &str = "1.2.840.10045.4.3.4";
const OID_CURVE_P256: &str = "1.2.840.10045.3.1.7";
const OID_CURVE_P384: &str = "1.3.132.0.34";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyboxRootType {
    Google,
    AospEc,
    AospRsa,
    Knox,
    Unknown,
}

impl KeyboxRootType {
    pub fn as_snake_case(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::AospEc => "aosp_ec",
            Self::AospRsa => "aosp_rsa",
            Self::Knox => "knox",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevocationSource {
    Online,
    Embedded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyMatch {
    Matched,
    Mismatched,
    Skipped,
}

#[derive(Debug, Serialize)]
pub struct KeyReport {
    pub keybox_index: usize,
    pub key_index: usize,
    pub algorithm: String,
    pub device_id: String,
    pub leaf_serial: String,
    pub leaf_subject: String,
    pub not_before: String,
    pub not_after: String,
    pub validity_ok: bool,
    pub chain_valid: bool,
    pub chain_len: usize,
    pub root_type: KeyboxRootType,
    pub key_match: KeyMatch,
    pub revocation_reason: Option<String>,
    pub revocation_serial: Option<String>,
    pub errors: Vec<String>,
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub revocation_source: RevocationSource,
    pub revocation_online_error: Option<String>,
    pub keys: Vec<KeyReport>,
    pub errors: Vec<String>,
}

pub fn validate_full(data: &[u8]) -> Result<ValidationReport> {
    if data.is_empty() {
        bail!("keybox data is empty");
    }
    if data.len() > MAX_KEYBOX_BYTES {
        bail!("keybox data exceeds {MAX_KEYBOX_BYTES} byte cap ({} bytes)", data.len());
    }
    let xml = std::str::from_utf8(data).context("keybox data is not valid UTF-8")?;

    let attestation: xml_model::AndroidAttestation =
        quick_xml::de::from_str(xml).context("keybox XML parse failed")?;
    if attestation.keyboxes.is_empty() {
        bail!("no <Keybox> elements found");
    }

    let revocation = load_revocation();
    let entries_map = revocation
        .data
        .get("entries")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut keys = Vec::new();
    for (kb_idx, keybox) in attestation.keyboxes.iter().enumerate() {
        for (k_idx, key) in keybox.keys.iter().enumerate() {
            keys.push(check_key(kb_idx + 1, k_idx + 1, keybox, key, &entries_map));
        }
    }
    if keys.is_empty() {
        bail!("no <Key> elements found in any <Keybox>");
    }

    let ok = keys.iter().any(|k| k.ok);
    Ok(ValidationReport {
        ok,
        revocation_source: revocation.source,
        revocation_online_error: revocation.online_error,
        keys,
        errors: Vec::new(),
    })
}

pub fn validate(data: &[u8]) -> Result<()> {
    let report = validate_full(data)?;
    if report.ok {
        return Ok(());
    }
    let messages: Vec<String> = report
        .keys
        .iter()
        .filter(|k| !k.ok)
        .map(|k| format!("Keybox#{}/Key#{} ({}): {}", k.keybox_index, k.key_index, k.algorithm, k.errors.join("; ")))
        .collect();
    bail!("keybox validation failed: {}", messages.join(" | "))
}

pub fn validate_file(path: &Path) -> Result<()> {
    let data = std::fs::read(path)
        .with_context(|| format!("reading keybox file {}", path.display()))?;
    validate(&data)
}

pub fn validate_file_full(path: &Path) -> Result<ValidationReport> {
    let data = std::fs::read(path)
        .with_context(|| format!("reading keybox file {}", path.display()))?;
    validate_full(&data)
}

mod xml_model {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct AndroidAttestation {
        #[serde(rename = "Keybox", default)]
        pub keyboxes: Vec<Keybox>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Keybox {
        #[serde(rename = "@DeviceID", default)]
        pub device_id: String,
        #[serde(rename = "Key", default)]
        pub keys: Vec<Key>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Key {
        #[serde(rename = "@algorithm", default)]
        pub algorithm: String,
        #[serde(rename = "PrivateKey", default)]
        pub private_key: Option<TextNode>,
        #[serde(rename = "CertificateChain", default)]
        pub chain: Option<CertificateChain>,
    }

    #[derive(Debug, Deserialize)]
    pub struct TextNode {
        #[serde(rename = "$text", default)]
        pub text: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct CertificateChain {
        #[serde(rename = "NumberOfCertificates", default)]
        pub declared: Option<TextNode>,
        #[serde(rename = "Certificate", default)]
        pub certificates: Vec<Certificate>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Certificate {
        #[serde(rename = "@format", default)]
        pub format: String,
        #[serde(rename = "$text", default)]
        pub text: String,
    }
}

fn check_key(
    keybox_index: usize,
    key_index: usize,
    keybox: &xml_model::Keybox,
    key: &xml_model::Key,
    entries: &serde_json::Map<String, serde_json::Value>,
) -> KeyReport {
    let device_id = if keybox.device_id.is_empty() {
        "Unknown".to_string()
    } else {
        keybox.device_id.clone()
    };
    let algorithm = if key.algorithm.is_empty() {
        "Unknown".to_string()
    } else {
        key.algorithm.clone()
    };

    let mut report = KeyReport {
        keybox_index,
        key_index,
        algorithm: algorithm.clone(),
        device_id: device_id.clone(),
        leaf_serial: String::new(),
        leaf_subject: String::new(),
        not_before: String::new(),
        not_after: String::new(),
        validity_ok: false,
        chain_valid: false,
        chain_len: 0,
        root_type: KeyboxRootType::Unknown,
        key_match: KeyMatch::Skipped,
        revocation_reason: None,
        revocation_serial: None,
        errors: Vec::new(),
        ok: false,
    };

    let chain = match &key.chain {
        Some(c) => c,
        None => {
            report.errors.push("missing CertificateChain".to_string());
            return report;
        }
    };

    let pem_certs: Vec<String> = chain
        .certificates
        .iter()
        .filter(|c| c.format.eq_ignore_ascii_case("pem"))
        .map(|c| c.text.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if pem_certs.is_empty() {
        report.errors.push("no Certificate[@format=\"pem\"] entries in chain".to_string());
        return report;
    }

    let der_certs = match decode_pem_chain(&pem_certs) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("PEM decode failed: {e}"));
            return report;
        }
    };
    report.chain_len = der_certs.len();

    if let Some(declared) = chain
        .declared
        .as_ref()
        .and_then(|n| n.text.trim().parse::<usize>().ok())
    {
        if declared != der_certs.len() {
            report.errors.push(format!(
                "NumberOfCertificates declared={declared} differs from actual={}",
                der_certs.len()
            ));
        }
    }

    let parsed: Result<Vec<X509Certificate<'_>>> = der_certs
        .iter()
        .enumerate()
        .map(|(idx, der)| {
            X509Certificate::from_der(der)
                .map(|(_, cert)| cert)
                .map_err(|e| anyhow!("certificate #{} ASN.1 parse failed: {}", idx + 1, e))
        })
        .collect();
    let certs = match parsed {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(e.to_string());
            return report;
        }
    };

    let (Some(leaf), Some(root)) = (certs.first(), certs.last()) else {
        report.errors.push("certificate chain is empty after parsing".to_string());
        return report;
    };

    report.leaf_serial = format!("{:x}", leaf.tbs_certificate.serial);
    report.leaf_subject = leaf.subject().to_string();
    report.not_before = leaf.validity().not_before.to_string();
    report.not_after = leaf.validity().not_after.to_string();
    report.validity_ok = check_validity(leaf, &mut report.errors);

    if certs.len() < 2 {
        report
            .errors
            .push("certificate chain has fewer than 2 entries (needs leaf + root)".to_string());
    } else {
        match verify_chain(&certs) {
            Ok(()) => report.chain_valid = true,
            Err(e) => report.errors.push(format!("chain verification failed: {e}")),
        }
    }

    report.root_type = detect_root_type(root);
    if report.root_type == KeyboxRootType::Unknown {
        report
            .errors
            .push("root certificate not recognized (Google/AOSP-EC/AOSP-RSA/Knox)".to_string());
    } else if let Err(e) = verify_link(root, root) {
        report
            .errors
            .push(format!("root self-signature failed: {e}"));
        report.chain_valid = false;
    }

    report.key_match = match key.private_key.as_ref().map(|n| n.text.trim()) {
        Some(pem) if !pem.is_empty() => match_private_key(pem, leaf, &mut report.errors),
        _ => KeyMatch::Skipped,
    };

    if let Some((sn, reason)) = lookup_revocation(&certs, entries) {
        report.revocation_serial = Some(sn);
        report.revocation_reason = Some(reason);
    }

    report.ok = report.chain_valid
        && report.validity_ok
        && report.root_type != KeyboxRootType::Unknown
        && report.key_match != KeyMatch::Mismatched;

    report
}

fn check_validity(leaf: &X509Certificate<'_>, errors: &mut Vec<String>) -> bool {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now_secs < PRE_NTP_EPOCH_SECS {
        errors.push("system clock pre-2020, skipping cert validity check".to_string());
        return true;
    }
    let now = ASN1Time::now();
    let v = leaf.validity();
    v.not_before <= now && now <= v.not_after
}

fn verify_chain(certs: &[X509Certificate<'_>]) -> Result<()> {
    for (idx, window) in certs.windows(2).enumerate() {
        let son = &window[0];
        let father = &window[1];
        verify_link(son, father)
            .with_context(|| format!("hop {} -> {}", idx, idx + 1))?;
    }
    Ok(())
}

fn verify_link(son: &X509Certificate<'_>, father: &X509Certificate<'_>) -> Result<()> {
    let issuer_raw = son.tbs_certificate.issuer.as_raw();
    let subject_raw = father.tbs_certificate.subject.as_raw();
    if issuer_raw != subject_raw {
        bail!(
            "issuer/subject DER mismatch: son issuer={}, father subject={}",
            son.issuer(),
            father.subject()
        );
    }
    let sig_alg_oid = son.signature_algorithm.algorithm.to_id_string();
    let tbs = son.tbs_certificate.as_ref();
    let sig = son.signature_value.data.as_ref();
    let father_pubkey_inner = father.public_key().subject_public_key.data.as_ref();
    let father_pubkey_alg = &father.public_key().algorithm;

    verify_signature(&sig_alg_oid, father_pubkey_alg, father_pubkey_inner, tbs, sig)
}

fn verify_signature(
    sig_alg_oid: &str,
    pubkey_alg: &AlgorithmIdentifier<'_>,
    pubkey_inner: &[u8],
    tbs: &[u8],
    sig: &[u8],
) -> Result<()> {
    use ring::signature;

    let do_verify = |alg: &'static dyn signature::VerificationAlgorithm| -> Result<()> {
        signature::UnparsedPublicKey::new(alg, pubkey_inner)
            .verify(tbs, sig)
            .map_err(|_| anyhow!("signature verification failed under {sig_alg_oid}"))
    };

    match sig_alg_oid {
        OID_RSA_SHA1 => do_verify(&signature::RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY),
        OID_RSA_SHA256 => do_verify(&signature::RSA_PKCS1_2048_8192_SHA256),
        OID_RSA_SHA384 => do_verify(&signature::RSA_PKCS1_2048_8192_SHA384),
        OID_RSA_SHA512 => do_verify(&signature::RSA_PKCS1_2048_8192_SHA512),
        OID_ECDSA_SHA256 => match curve_oid(pubkey_alg).as_deref() {
            Some(OID_CURVE_P256) => do_verify(&signature::ECDSA_P256_SHA256_ASN1),
            Some(other) => bail!("ECDSA-SHA256 with unsupported curve OID {other}"),
            None => bail!("ECDSA-SHA256 missing curve OID"),
        },
        OID_ECDSA_SHA384 => match curve_oid(pubkey_alg).as_deref() {
            Some(OID_CURVE_P384) => do_verify(&signature::ECDSA_P384_SHA384_ASN1),
            Some(other) => bail!("ECDSA-SHA384 with unsupported curve OID {other}"),
            None => bail!("ECDSA-SHA384 missing curve OID"),
        },
        OID_ECDSA_SHA1 => bail!("ECDSA-SHA1 not supported (ring lacks the verifier)"),
        OID_ECDSA_SHA512 => bail!("ECDSA-SHA512 not supported (ring lacks the verifier)"),
        other => bail!("unsupported signature algorithm OID {other}"),
    }
}

fn curve_oid(alg: &AlgorithmIdentifier<'_>) -> Option<String> {
    let params = alg.parameters.as_ref()?;
    params.as_oid().ok().map(|oid| oid.to_id_string())
}

fn detect_root_type(root: &X509Certificate<'_>) -> KeyboxRootType {
    let root_inner = root.public_key().subject_public_key.data.as_ref();
    for (kind, anchor_inner) in trust_anchors() {
        if anchor_inner.as_slice() == root_inner {
            return *kind;
        }
    }
    KeyboxRootType::Unknown
}

fn trust_anchors() -> &'static [(KeyboxRootType, Vec<u8>)] {
    static CELL: OnceLock<Vec<(KeyboxRootType, Vec<u8>)>> = OnceLock::new();
    CELL.get_or_init(|| {
        let pairs = [
            (KeyboxRootType::Google, ROOT_GOOGLE_PEM),
            (KeyboxRootType::AospEc, ROOT_AOSP_EC_PEM),
            (KeyboxRootType::AospRsa, ROOT_AOSP_RSA_PEM),
            (KeyboxRootType::Knox, ROOT_KNOX_PEM),
        ];
        pairs
            .iter()
            .filter_map(|(kind, pem_bytes)| match parse_anchor(pem_bytes) {
                Ok(inner) => Some((*kind, inner)),
                Err(e) => {
                    warn!("trust anchor {kind:?} disabled: {e}");
                    None
                }
            })
            .collect()
    })
}

fn parse_anchor(pem_bytes: &[u8]) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(pem_bytes).context("anchor PEM is not valid UTF-8")?;
    let der = decode_pem_body(text).context("anchor PEM body decode failed")?;
    let (_, spki) =
        SubjectPublicKeyInfo::from_der(&der).map_err(|e| anyhow!("anchor SPKI parse: {e}"))?;
    Ok(spki.subject_public_key.data.to_vec())
}

fn match_private_key(
    private_pem: &str,
    leaf: &X509Certificate<'_>,
    errors: &mut Vec<String>,
) -> KeyMatch {
    let leaf_spki = leaf.public_key().raw;
    let cleaned = strip_pem_indent(private_pem);

    if let Some(verdict) = try_match_rsa(&cleaned, leaf_spki, errors) {
        return verdict;
    }
    if let Some(verdict) = try_match_p256(&cleaned, leaf_spki, errors) {
        return verdict;
    }
    if let Some(verdict) = try_match_p384(&cleaned, leaf_spki, errors) {
        return verdict;
    }

    errors.push("private key present but unparseable (encrypted PKCS#8, unsupported curve, or corrupt)".to_string());
    KeyMatch::Mismatched
}

fn try_match_rsa(pem: &str, leaf_spki: &[u8], errors: &mut Vec<String>) -> Option<KeyMatch> {
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
    use rsa::{RsaPrivateKey, RsaPublicKey};

    let priv_key = RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .ok()?;
    let pub_key = RsaPublicKey::from(&priv_key);
    match pub_key.to_public_key_der() {
        Ok(der) => Some(verdict(der.as_bytes(), leaf_spki)),
        Err(e) => {
            errors.push(format!("RSA public-side DER encode failed: {e}"));
            Some(KeyMatch::Mismatched)
        }
    }
}

fn try_match_p256(pem: &str, leaf_spki: &[u8], errors: &mut Vec<String>) -> Option<KeyMatch> {
    use p256::pkcs8::{DecodePrivateKey, EncodePublicKey};
    let priv_key = p256::SecretKey::from_pkcs8_pem(pem).ok()?;
    let pub_key = priv_key.public_key();
    match pub_key.to_public_key_der() {
        Ok(der) => Some(verdict(der.as_bytes(), leaf_spki)),
        Err(e) => {
            errors.push(format!("P-256 public-side DER encode failed: {e}"));
            Some(KeyMatch::Mismatched)
        }
    }
}

fn try_match_p384(pem: &str, leaf_spki: &[u8], errors: &mut Vec<String>) -> Option<KeyMatch> {
    use p384::pkcs8::{DecodePrivateKey, EncodePublicKey};
    let priv_key = p384::SecretKey::from_pkcs8_pem(pem).ok()?;
    let pub_key = priv_key.public_key();
    match pub_key.to_public_key_der() {
        Ok(der) => Some(verdict(der.as_bytes(), leaf_spki)),
        Err(e) => {
            errors.push(format!("P-384 public-side DER encode failed: {e}"));
            Some(KeyMatch::Mismatched)
        }
    }
}

fn verdict(derived_spki: &[u8], leaf_spki: &[u8]) -> KeyMatch {
    if derived_spki == leaf_spki {
        KeyMatch::Matched
    } else {
        KeyMatch::Mismatched
    }
}

fn lookup_revocation(
    certs: &[X509Certificate<'_>],
    entries: &serde_json::Map<String, serde_json::Value>,
) -> Option<(String, String)> {
    if entries.is_empty() {
        return None;
    }
    let leaf = certs.first()?;
    let sn = format!("{:x}", leaf.tbs_certificate.serial);
    let entry = entries.get(&sn)?;
    let reason = entry
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("UNKNOWN")
        .to_string();
    Some((sn, reason))
}

#[derive(Clone)]
struct Revocation {
    source: RevocationSource,
    online_error: Option<String>,
    data: serde_json::Value,
}

fn load_revocation() -> Revocation {
    static CACHE: OnceLock<Mutex<Option<(Instant, Revocation)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((stored_at, rev)) = guard.as_ref() {
            if stored_at.elapsed() < REVOCATION_CACHE_TTL {
                return rev.clone();
            }
        }
    }
    let fresh = match fetch_revocation_online() {
        Ok(json) => Revocation {
            source: RevocationSource::Online,
            online_error: None,
            data: json,
        },
        Err(e) => {
            warn!("revocation online fetch failed, using embedded: {e}");
            let data = serde_json::from_slice(EMBEDDED_REVOCATION)
                .unwrap_or_else(|_| serde_json::json!({"entries": {}}));
            Revocation {
                source: RevocationSource::Embedded,
                online_error: Some(e.to_string()),
                data,
            }
        }
    };
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((Instant::now(), fresh.clone()));
    }
    fresh
}

fn fetch_revocation_online() -> Result<serde_json::Value> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before epoch")?
        .as_secs();
    let url = format!("{GOOGLE_STATUS_URL}?ts={ts}");
    let agent = ureq::AgentBuilder::new()
        .timeout(REVOCATION_TIMEOUT)
        .build();
    let resp = agent
        .get(&url)
        .set("Cache-Control", "max-age=0, no-cache, no-store, must-revalidate")
        .set("Pragma", "no-cache")
        .set("Expires", "0")
        .call()
        .context("revocation HTTP GET failed")?;
    let body = resp.into_string().context("revocation body read failed")?;
    serde_json::from_str(&body).context("revocation JSON parse failed")
}

fn decode_pem_chain(pems: &[String]) -> Result<Vec<Vec<u8>>> {
    pems.iter()
        .enumerate()
        .map(|(idx, pem)| {
            decode_pem_body(pem)
                .with_context(|| format!("certificate #{} PEM decode", idx + 1))
        })
        .collect()
}

fn decode_pem_body(pem: &str) -> Result<Vec<u8>> {
    let mut in_body = false;
    let mut started = false;
    let mut body = String::new();
    for line in pem.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("-----BEGIN") {
            started = true;
            in_body = true;
            continue;
        }
        if trimmed.starts_with("-----END") {
            break;
        }
        if in_body && !trimmed.is_empty() {
            if body.len() + trimmed.len() > MAX_PEM_BODY_BYTES {
                bail!("PEM body exceeds {MAX_PEM_BODY_BYTES} byte cap");
            }
            body.push_str(trimmed);
        }
    }
    if !started {
        bail!("no PEM header found");
    }
    base64::engine::general_purpose::STANDARD
        .decode(body.as_bytes())
        .context("PEM body base64 decode")
}

fn strip_pem_indent(pem: &str) -> String {
    pem.lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_rejected() {
        assert!(validate(&[]).is_err());
    }

    #[test]
    fn oversized_data_rejected() {
        let huge = vec![0u8; MAX_KEYBOX_BYTES + 1];
        let err = validate(&huge).unwrap_err().to_string();
        assert!(err.contains("byte cap"), "expected size cap error, got: {err}");
    }

    #[test]
    fn non_utf8_rejected() {
        assert!(validate(&[0xff, 0xfe, 0xfd]).is_err());
    }

    #[test]
    fn missing_keybox_rejected() {
        let xml = b"<?xml version=\"1.0\"?><AndroidAttestation></AndroidAttestation>";
        assert!(validate(xml).is_err());
    }

    #[test]
    fn trust_anchors_load() {
        let anchors = trust_anchors();
        assert_eq!(anchors.len(), 4, "all 4 trust anchors must parse");
        for (kind, inner) in anchors {
            assert!(!inner.is_empty(), "anchor {kind:?} has empty pubkey inner");
        }
    }

    #[test]
    fn embedded_revocation_parses() {
        let v: serde_json::Value =
            serde_json::from_slice(EMBEDDED_REVOCATION).expect("embedded status.json parses");
        assert!(v.get("entries").is_some(), "embedded status.json has entries field");
    }

    #[test]
    fn pem_decode_strips_armor() {
        let pem = "-----BEGIN CERTIFICATE-----\nAAEC\n-----END CERTIFICATE-----\n";
        assert_eq!(decode_pem_body(pem).unwrap(), vec![0x00, 0x01, 0x02]);
    }

    #[test]
    fn pem_decode_rejects_missing_header() {
        assert!(decode_pem_body("not pem").is_err());
    }

    #[test]
    fn pem_decode_rejects_oversized_body() {
        let huge = "-".repeat(MAX_PEM_BODY_BYTES + 100);
        let pem = format!("-----BEGIN X-----\n{huge}\n-----END X-----\n");
        assert!(decode_pem_body(&pem).is_err());
    }

    #[test]
    fn pem_indent_stripper_normalizes() {
        let input = "    line1\n  line2\nline3";
        assert_eq!(strip_pem_indent(input), "line1\nline2\nline3");
    }

    #[test]
    fn root_type_snake_case_consistent() {
        assert_eq!(KeyboxRootType::Google.as_snake_case(), "google");
        assert_eq!(KeyboxRootType::AospEc.as_snake_case(), "aosp_ec");
        assert_eq!(KeyboxRootType::AospRsa.as_snake_case(), "aosp_rsa");
        assert_eq!(KeyboxRootType::Knox.as_snake_case(), "knox");
        assert_eq!(KeyboxRootType::Unknown.as_snake_case(), "unknown");
    }

    #[test]
    fn ecdsa_p256_self_signed_link_verifies() {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "test-leaf");
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("rcgen key");
        let cert = params.self_signed(&key).expect("rcgen build");
        let der = cert.der();

        let (_, parsed) = X509Certificate::from_der(der.as_ref()).expect("x509 parse");
        verify_link(&parsed, &parsed).expect("self-signed P-256 link verifies");
    }

    #[test]
    fn unsupported_signature_oid_emits_specific_error() {
        let dummy_alg = AlgorithmIdentifier {
            algorithm: oid_registry::OID_SIG_ED25519,
            parameters: None,
        };
        let err = verify_signature("1.3.101.112", &dummy_alg, &[0u8; 32], b"tbs", b"sig")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("unsupported signature algorithm OID 1.3.101.112"),
            "expected unsupported-OID error, got: {err}"
        );
    }

    #[test]
    fn ecdsa_sha1_emits_ring_limit_error() {
        let dummy_alg = AlgorithmIdentifier {
            algorithm: oid_registry::OID_SIG_ED25519,
            parameters: None,
        };
        let err = verify_signature(OID_ECDSA_SHA1, &dummy_alg, &[0u8; 32], b"tbs", b"sig")
            .unwrap_err()
            .to_string();
        assert!(err.contains("ECDSA-SHA1 not supported"), "got: {err}");
    }

    #[test]
    fn lookup_revocation_finds_serial() {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256, SerialNumber};

        let mut params = CertificateParams::default();
        params.serial_number = Some(SerialNumber::from(0xdead_beefu64));
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "rev-test");
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("rcgen key");
        let cert = params.self_signed(&key).expect("rcgen build");
        let der = cert.der();
        let (_, parsed) = X509Certificate::from_der(der.as_ref()).expect("x509 parse");

        let mut entries = serde_json::Map::new();
        entries.insert(
            "deadbeef".to_string(),
            serde_json::json!({"reason": "KEY_COMPROMISE"}),
        );

        let result = lookup_revocation(std::slice::from_ref(&parsed), &entries);
        assert_eq!(
            result,
            Some(("deadbeef".to_string(), "KEY_COMPROMISE".to_string()))
        );
    }

    #[test]
    fn lookup_revocation_returns_none_when_clean() {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256, SerialNumber};

        let mut params = CertificateParams::default();
        params.serial_number = Some(SerialNumber::from(0x1234u64));
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, "clean");
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("rcgen key");
        let cert = params.self_signed(&key).expect("rcgen build");
        let der = cert.der();
        let (_, parsed) = X509Certificate::from_der(der.as_ref()).expect("x509 parse");

        let mut entries = serde_json::Map::new();
        entries.insert(
            "deadbeef".to_string(),
            serde_json::json!({"reason": "KEY_COMPROMISE"}),
        );

        assert_eq!(
            lookup_revocation(std::slice::from_ref(&parsed), &entries),
            None
        );
    }
}
