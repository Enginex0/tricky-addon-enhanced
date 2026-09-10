use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;

use std::path::Path;
use tracing::info;

use crate::platform::fs::atomic_write;

const TARGET_KEYBOX: &str = "/data/adb/tricky_store/keybox.xml";
const BACKUP_KEYBOX: &str = "/data/adb/tricky_store/keybox.xml.bak";

pub fn generate_and_install() -> Result<()> {
    let xml = generate()?;

    if Path::new(TARGET_KEYBOX).exists() {
        std::fs::copy(TARGET_KEYBOX, BACKUP_KEYBOX)
            .context("failed to backup existing keybox")?;
    }

    atomic_write(Path::new(TARGET_KEYBOX), xml.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(TARGET_KEYBOX, std::fs::Permissions::from_mode(0o644));
    }

    info!("device keybox generated and installed");
    Ok(())
}

fn generate() -> Result<String> {
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, "Android Keybox");

    let ec_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .context("EC key generation failed")?;
    let cert = params
        .self_signed(&ec_key)
        .context("EC cert generation failed")?;
    let ec_pem = ec_key.serialize_pem();
    let cert_pem = cert.pem();

    let mut rng = rand::rngs::OsRng;
    let rsa_key = RsaPrivateKey::new(&mut rng, 2048)
        .context("RSA 2048 keygen failed")?;
    let rsa_pem = rsa_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .context("RSA PEM encoding failed")?;

    Ok(build_xml(&ec_pem, &cert_pem, rsa_pem.as_ref()))
}

fn build_xml(ec_key: &str, cert: &str, rsa_key: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
    <AndroidAttestation>
        <NumberOfKeyboxes>1</NumberOfKeyboxes>
        <Keybox DeviceID="sw">
            <Key algorithm="ecdsa">
                <PrivateKey format="pem">
{}
                </PrivateKey>
                <CertificateChain>
                    <NumberOfCertificates>1</NumberOfCertificates>
                        <Certificate format="pem">
{}
                        </Certificate>
                </CertificateChain>
            </Key>
            <Key algorithm="rsa">
                <PrivateKey format="pem">
{}
                </PrivateKey>
            </Key>
        </Keybox>
</AndroidAttestation>"#,
        indent(ec_key, 20),
        indent(cert, 24),
        indent(rsa_key, 20),
    )
}

fn indent(pem: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    pem.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
