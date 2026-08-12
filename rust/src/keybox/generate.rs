use anyhow::{Context, Result};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, PKCS_ECDSA_P256_SHA256};
use rsa::pkcs8::EncodePrivateKey;
use rsa::RsaPrivateKey;

use tracing::info;

use crate::platform::fs::atomic_write;

pub fn generate_and_install() -> Result<()> {
    let xml = generate()?;
    let engine = crate::engine::Engine::detect();
    let target = engine.keybox_path()?;
    let backup = target.with_extension("xml.bak");

    if target.exists() {
        std::fs::copy(&target, &backup).context("failed to backup existing keybox")?;
    }

    atomic_write(&target, xml.as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if engine == crate::engine::Engine::TrickyStore {
            0o644
        } else {
            0o600
        };
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode));
    }

    info!("device keybox generated and installed");
    Ok(())
}

fn generate() -> Result<String> {
    let mut params = CertificateParams::default();
    params.alg = &PKCS_ECDSA_P256_SHA256;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "Android Keybox");

    let cert = Certificate::from_params(params).context("EC cert generation failed")?;
    let ec_pem = cert.serialize_private_key_pem();
    let cert_pem = cert.serialize_pem().context("cert serialization failed")?;

    let mut rng = rand::rngs::OsRng;
    let rsa_key = RsaPrivateKey::new(&mut rng, 2048).context("RSA 2048 keygen failed")?;
    let rsa_pem = rsa_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
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
