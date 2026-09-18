//! TLS material: a generated self-signed identity, or a certificate and key
//! the user supplied.
//!
//! Self-signed certificates are written into Kog's config directory and then
//! reused, so the fingerprint a user trusted once keeps working across
//! restarts. Importing a user certificate copies it (and its key) into the
//! same directory with owner-only permissions rather than reading it from
//! wherever it happened to live.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::config::{TlsMode, TlsSettings};

/// Everything needed to start a TLS listener.
pub struct TlsMaterial {
    pub certificate_chain: Vec<CertificateDer<'static>>,
    pub private_key: PrivateKeyDer<'static>,
    /// Where the certificate lives, so the UI can offer "export/trust this".
    pub certificate_path: PathBuf,
}

/// Directory holding Kog's TLS files.
fn tls_dir() -> Result<PathBuf, String> {
    let settings = kog_audio::settings::setting_path("server.json")
        .ok_or_else(|| "Kog's settings directory is unavailable".to_owned())?;
    let parent = settings
        .parent()
        .ok_or_else(|| "Kog's settings directory is unavailable".to_owned())?;
    Ok(parent.join("tls"))
}

/// Names the generated certificate is valid for. The bind address is what
/// clients will actually type, so it must be in the SAN list.
fn self_signed_names(bind: SocketAddr) -> Vec<String> {
    let mut names = vec!["localhost".to_owned()];
    let address = bind.ip().to_string();
    if !names.contains(&address) {
        names.push(address);
    }
    names
}

/// Build the TLS configuration for the given settings.
pub fn server_config(settings: &TlsSettings, bind: SocketAddr) -> Result<rustls::ServerConfig, String> {
    let material = load_material(settings, bind)?;
    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(material.certificate_chain, material.private_key)
        .map_err(|error| format!("preparing the TLS listener: {error}"))
}

/// Load (or generate) the certificate and key implied by `settings`.
pub fn load_material(settings: &TlsSettings, bind: SocketAddr) -> Result<TlsMaterial, String> {
    match settings.mode {
        TlsMode::Off => Err("TLS is not enabled".to_owned()),
        TlsMode::SelfSigned => self_signed(bind),
        TlsMode::Pem => {
            if settings.certificate_path.as_os_str().is_empty()
                || settings.private_key_path.as_os_str().is_empty()
            {
                return Err("Choose both a certificate and a private key".to_owned());
            }
            load_pem(&settings.certificate_path, &settings.private_key_path)
        }
    }
}

fn self_signed(bind: SocketAddr) -> Result<TlsMaterial, String> {
    let directory = tls_dir()?;
    let certificate_path = directory.join("self-signed.pem");
    let key_path = directory.join("self-signed.key");
    let names = self_signed_names(bind);

    // Reuse an existing pair so a trusted fingerprint stays stable. The
    // covered names are recorded beside it rather than parsed back out of the
    // certificate: an IP address is encoded as a binary SAN, so sniffing the
    // DER for text silently misses it.
    let names_path = directory.join("self-signed.names");
    if certificate_path.is_file() && key_path.is_file() {
        let recorded = std::fs::read_to_string(&names_path).unwrap_or_default();
        let same_names = recorded.lines().map(str::trim).eq(names.iter().map(String::as_str));
        if same_names && let Ok(material) = load_pem(&certificate_path, &key_path) {
            return Ok(TlsMaterial {
                certificate_path,
                ..material
            });
        }
    }

    let certified = rcgen::generate_simple_self_signed(names.clone())
        .map_err(|error| format!("generating a self-signed certificate: {error}"))?;
    let certificate_pem = certified.cert.pem();
    let key_pem = certified.key_pair.serialize_pem();
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("creating {}: {error}", directory.display()))?;
    write_private(&certificate_path, certificate_pem.as_bytes())?;
    write_private(&key_path, key_pem.as_bytes())?;
    let mut recorded = names.join("\n");
    recorded.push('\n');
    write_private(&names_path, recorded.as_bytes())?;
    load_pem(&certificate_path, &key_path)
}

fn load_pem(certificate_path: &Path, key_path: &Path) -> Result<TlsMaterial, String> {
    let certificate_bytes = std::fs::read(certificate_path)
        .map_err(|error| format!("reading {}: {error}", certificate_path.display()))?;
    let mut reader = std::io::BufReader::new(certificate_bytes.as_slice());
    let certificate_chain: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                format!(
                    "reading certificates from {}: {error}",
                    certificate_path.display()
                )
            })?;
    if certificate_chain.is_empty() {
        return Err(format!(
            "{} contains no certificates",
            certificate_path.display()
        ));
    }

    let key_bytes = std::fs::read(key_path)
        .map_err(|error| format!("reading {}: {error}", key_path.display()))?;
    let mut reader = std::io::BufReader::new(key_bytes.as_slice());
    let private_key = rustls_pemfile::private_key(&mut reader)
        .map_err(|error| format!("reading the private key from {}: {error}", key_path.display()))?
        .ok_or_else(|| format!("{} contains no private key", key_path.display()))?;

    Ok(TlsMaterial {
        certificate_chain,
        private_key,
        certificate_path: certificate_path.to_path_buf(),
    })
}

/// Copy a certificate and key the user chose into Kog's TLS directory and
/// return settings pointing at the copies, so the server does not depend on
/// files the user may later move or restrict.
pub fn import_pem_pair(certificate_source: &Path, key_source: &Path) -> Result<TlsSettings, String> {
    // Validate before copying: a mismatched pair should fail here, not at
    // startup.
    load_pem(certificate_source, key_source)?;
    let directory = tls_dir()?;
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("creating {}: {error}", directory.display()))?;
    let certificate_path = directory.join("imported-certificate.pem");
    let private_key_path = directory.join("imported-private-key.pem");
    let certificate_bytes = std::fs::read(certificate_source)
        .map_err(|error| format!("reading {}: {error}", certificate_source.display()))?;
    let key_bytes = std::fs::read(key_source)
        .map_err(|error| format!("reading {}: {error}", key_source.display()))?;
    write_private(&certificate_path, &certificate_bytes)?;
    write_private(&private_key_path, &key_bytes)?;
    Ok(TlsSettings {
        mode: TlsMode::Pem,
        certificate_path,
        private_key_path,
    })
}

/// Write with owner-only permissions: private keys must not be world readable.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("writing {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("writing {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TLS files live in the user's config directory, so tests point HOME at a
    /// tempdir and keep the two TLS tests serialized.
    fn with_temp_home<T>(body: impl FnOnce() -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", directory.path());
        }
        let result = body();
        unsafe {
            match previous {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
        result
    }

    fn bind() -> SocketAddr {
        "127.0.0.1:8420".parse().unwrap()
    }

    #[test]
    fn self_signed_is_reused_until_the_names_change() {
        with_temp_home(|| {
            let settings = TlsSettings {
                mode: TlsMode::SelfSigned,
                ..TlsSettings::default()
            };
            let first = load_material(&settings, bind()).expect("generate");
            let first_der = first.certificate_chain[0].clone().as_ref().to_vec();
            assert!(first.certificate_path.is_file());

            let second = load_material(&settings, bind()).expect("reuse");
            assert_eq!(
                second.certificate_chain[0].as_ref(),
                first_der.as_slice(),
                "an existing identity must be reused so a trusted fingerprint survives"
            );

            // A different bind address needs a different SAN, so the identity
            // is regenerated rather than silently serving a wrong name.
            let moved: SocketAddr = "192.168.1.10:8420".parse().unwrap();
            let third = load_material(&settings, moved).expect("regenerate");
            assert_ne!(
                third.certificate_chain[0].as_ref(),
                first_der.as_slice(),
                "changing the bind address must regenerate the certificate"
            );
        });
    }

    #[test]
    fn a_generated_identity_loads_into_a_rustls_config() {
        with_temp_home(|| {
            let settings = TlsSettings {
                mode: TlsMode::SelfSigned,
                ..TlsSettings::default()
            };
            server_config(&settings, bind()).expect("rustls accepts the generated pair");
        });
    }

    #[test]
    fn the_private_key_is_not_world_readable() {
        with_temp_home(|| {
            let settings = TlsSettings {
                mode: TlsMode::SelfSigned,
                ..TlsSettings::default()
            };
            load_material(&settings, bind()).expect("generate");
            let key_path = tls_dir().unwrap().join("self-signed.key");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&key_path).unwrap().permissions().mode();
                assert_eq!(mode & 0o077, 0, "key permissions were {mode:o}");
            }
        });
    }

    #[test]
    fn importing_copies_the_pair_into_the_settings_directory() {
        with_temp_home(|| {
            // Generate a pair, then import it from an unrelated location.
            let source_dir = tempfile::tempdir().unwrap();
            let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
            let cert = source_dir.path().join("cert.pem");
            let key = source_dir.path().join("key.pem");
            std::fs::write(&cert, generated.cert.pem()).unwrap();
            std::fs::write(&key, generated.key_pair.serialize_pem()).unwrap();

            let settings = import_pem_pair(&cert, &key).expect("import");
            assert_eq!(settings.mode, TlsMode::Pem);
            assert_ne!(settings.certificate_path, cert, "copied, not referenced");
            assert!(settings.certificate_path.starts_with(tls_dir().unwrap()));
            load_material(&settings, bind()).expect("imported pair loads");
        });
    }

    #[test]
    fn a_mismatched_or_missing_pair_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        // Match rather than expect_err: TlsMaterial holds a private key and
        // deliberately has no Debug impl, so it cannot be formatted on panic.
        let missing = directory.path().join("nope.pem");
        let error = match load_pem(&missing, &missing) {
            Ok(_) => panic!("missing files should not load"),
            Err(error) => error,
        };
        assert!(error.contains("nope.pem"), "{error}");

        let empty = directory.path().join("empty.pem");
        std::fs::write(&empty, b"not a pem").unwrap();
        let error = match load_pem(&empty, &empty) {
            Ok(_) => panic!("garbage should not load"),
            Err(error) => error,
        };
        assert!(!error.is_empty());
    }
}
