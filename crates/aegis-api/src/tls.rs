//! TLS for the API (docs/specs/api.md, Encryption): Aegis's own authority, made once and kept
//! beside the settings, and the certificate it issues for this computer's addresses; or the
//! person's certificate. rustls with ring's crypto, HTTP/1.1 only.

use crate::store::{folder, private_write};
use ring::digest::{SHA256, digest};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio_rustls::rustls;
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_rustls::rustls::pki_types::pem::{Error as PemError, PemObject};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::server::{ClientHello, ResolvesServerCert, WantsServerCert};
use tokio_rustls::rustls::sign::CertifiedKey;

/// A certificate chain and the key that signs for it. No `Debug`: it holds the key.
pub struct Certificate {
    pub chain: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

/// How long Aegis's authority lasts, in days.
const ROOT_DAYS: i64 = 10 * 365;
/// How long a certificate it issues lasts, in days: under the 398 browsers take.
const LEAF_DAYS: i64 = 397;
/// A certificate is issued again this many days before it ends, even if nothing moved.
const LEAF_RENEW_DAYS: i64 = 30;
/// How often a handshake looks at this computer's addresses again, at most.
const LOOK_EVERY: Duration = Duration::from_secs(10);
/// The authority's name. Fixed, so the issuer of a certificate is rebuilt from the key alone.
const ROOT_NAME: &str = "Aegis local authority";

/// Aegis's authority: its certificate and the key that signs what it issues.
struct Authority {
    /// What it is, as an issuer: its name, its key's identifier.
    params: rcgen::CertificateParams,
    key: rcgen::KeyPair,
    der: CertificateDer<'static>,
}

/// Where Aegis's authority is kept: `<data>/api/ca.pem` and `ca-key.pem`.
fn root_paths(data: &Path) -> (PathBuf, PathBuf) {
    (folder(data).join("ca.pem"), folder(data).join("ca-key.pem"))
}

/// The authority's files, made when either is missing.
fn root_files(data: &Path) -> Result<(PathBuf, PathBuf), String> {
    let (cert, key) = root_paths(data);
    if !(cert.exists() && key.exists()) {
        make_root(&cert, &key)?;
    }
    Ok((cert, key))
}

/// Aegis's authority's certificate: what a phone installs, and what a client pins.
pub fn root(data: &Path) -> Result<CertificateDer<'static>, String> {
    let (cert, _) = root_files(data)?;
    CertificateDer::from_pem_file(&cert).map_err(|e| format!("{}: {e}", cert.display()))
}

/// Aegis's authority's certificate as PEM, as Save certificate… writes it.
pub fn root_pem(data: &Path) -> Result<String, String> {
    let (cert, _) = root_files(data)?;
    std::fs::read_to_string(&cert).map_err(|e| format!("{}: {e}", cert.display()))
}

/// A new authority, then `root`: clients that pinned the old one, and phones that installed it,
/// stop connecting.
pub fn renew(data: &Path) -> Result<CertificateDer<'static>, String> {
    let (cert, key) = root_paths(data);
    make_root(&cert, &key)?;
    root(data)
}

fn authority(data: &Path) -> Result<Authority, String> {
    let (_, key) = root_files(data)?;
    let der = root(data)?;
    let pem = std::fs::read_to_string(&key).map_err(|e| format!("{}: {e}", key.display()))?;
    let key = rcgen::KeyPair::from_pem(&pem).map_err(|e| format!("{}: {e}", key.display()))?;
    Ok(Authority {
        params: root_params(),
        key,
        der,
    })
}

/// The authority as an issuer: its name, and a CA that issues only server certificates for
/// `localhost` and private addresses. A phone that installs it trusts it for nothing else, so its
/// key, were it to leak, couldn't stand in for a site on the internet.
fn root_params() -> rcgen::CertificateParams {
    use rcgen::{CidrSubnet, GeneralSubtree};
    let mut params = rcgen::CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, ROOT_NAME);
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    let net = |address: [u8; 4], prefix| {
        GeneralSubtree::IpAddress(CidrSubnet::from_v4_prefix(address, prefix))
    };
    params.name_constraints = Some(rcgen::NameConstraints {
        permitted_subtrees: vec![
            GeneralSubtree::DnsName("localhost".into()),
            net([10, 0, 0, 0], 8),
            net([172, 16, 0, 0], 12),
            net([192, 168, 0, 0], 16),
            // Tailscale's.
            net([100, 64, 0, 0], 10),
            net([127, 0, 0, 0], 8),
            GeneralSubtree::IpAddress(CidrSubnet::from_v6_prefix(
                Ipv6Addr::LOCALHOST.octets(),
                128,
            )),
        ],
        excluded_subtrees: Vec::new(),
    });
    params
}

/// Writes a new authority, an ECDSA P-256 key valid ten years, and its key (private to the
/// person).
fn make_root(cert: &Path, key: &Path) -> Result<(), String> {
    let failed =
        |error: &dyn std::fmt::Display| format!("Aegis's authority couldn't be made: {error}");
    let pair =
        rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e| failed(&e))?;
    let mut params = root_params();
    valid(&mut params, today(), ROOT_DAYS);
    let certificate = params.self_signed(&pair).map_err(|e| failed(&e))?;
    private_write(key, pair.serialize_pem().as_bytes()).map_err(|e| failed(&e))?;
    private_write(cert, certificate.pem().as_bytes()).map_err(|e| failed(&e))
}

/// Today, counted in days from 1970-01-01.
fn today() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() / 86_400) as i64
}

/// Valid for `days` from the day before `today`: a phone's clock may run a little behind.
fn valid(params: &mut rcgen::CertificateParams, today: i64, days: i64) {
    let date = |days| {
        let (year, month, day) = civil(days);
        rcgen::date_time_ymd(year, month, day)
    };
    params.not_before = date(today - 1);
    params.not_after = date(today - 1 + days);
}

/// The calendar date of a day counted from 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted + 2) / 5 + 1) as u8;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    } as u8;
    let year = (year_of_era + era * 400) as i32 + i32::from(month <= 2);
    (year, month, day)
}

/// Tailscale's address space, `100.64/10`.
pub fn is_tailscale(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    first == 100 && second & 0xc0 == 64
}

/// A private IPv4 address a phone can reach this computer at: a LAN's (`10/8`, `172.16/12`,
/// `192.168/16`) or Tailscale's (`100.64/10`). Loopback is in every certificate anyway.
pub fn is_private(address: Ipv4Addr) -> bool {
    address.is_private() || is_tailscale(address)
}

/// This computer's private IPv4 addresses: Tailscale's first (an app a phone installs through one
/// works at home and away), then the others, each in order.
pub fn private_addresses() -> Vec<Ipv4Addr> {
    let networks = sysinfo::Networks::new_with_refreshed_list();
    let mut found: Vec<Ipv4Addr> = networks
        .list()
        .values()
        .flat_map(|network| network.ip_networks())
        .filter_map(|network| match network.addr {
            IpAddr::V4(address) if is_private(address) => Some(address),
            _ => None,
        })
        .collect();
    found.sort_by_key(|address| (!is_tailscale(*address), *address));
    found.dedup();
    found
}

/// A certificate the authority issued, and what it was issued for.
struct Issued {
    /// The certificate, then the authority's: a client that pins the authority finds it there.
    certified: Arc<CertifiedKey>,
    addresses: Vec<Ipv4Addr>,
    /// The day it is issued again even if nothing moved.
    renew: i64,
    /// When this computer's addresses were last looked at.
    looked: Instant,
}

impl Authority {
    /// A server certificate for `localhost`, loopback and `addresses`, valid `LEAF_DAYS` from
    /// `today`, with a key of its own. No subject: the names are in its alternative names alone,
    /// which are then critical, so no client reads a name from elsewhere.
    fn issue(&self, addresses: Vec<Ipv4Addr>, today: i64) -> Result<Issued, String> {
        let failed = |error: &dyn std::fmt::Display| {
            format!("Aegis's certificate couldn't be issued: {error}")
        };
        let mut names = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ];
        names.extend(addresses.iter().map(Ipv4Addr::to_string));
        let mut params = rcgen::CertificateParams::new(names).map_err(|e| failed(&e))?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;
        valid(&mut params, today, LEAF_DAYS);
        let pair =
            rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e| failed(&e))?;
        let issuer = rcgen::Issuer::from_params(&self.params, &self.key);
        let leaf = params.signed_by(&pair, &issuer).map_err(|e| failed(&e))?;
        let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(pair.serialize_der()));
        let key = provider()
            .key_provider
            .load_private_key(key)
            .map_err(|e| failed(&e))?;
        Ok(Issued {
            certified: Arc::new(CertifiedKey {
                cert: vec![leaf.der().clone(), self.der.clone()],
                key,
                ocsp: None,
            }),
            addresses,
            renew: today - 1 + LEAF_DAYS - LEAF_RENEW_DAYS,
            looked: Instant::now(),
        })
    }
}

/// What the server shows with Aegis's authority: a certificate issued when the server starts and
/// again when an address of this computer comes or goes (a new LAN, Tailscale turned on) or the
/// certificate nears its end. Nothing of it is written: the authority issues another at each
/// start.
struct Own {
    authority: Authority,
    issued: Mutex<Issued>,
}

impl std::fmt::Debug for Own {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Own").finish_non_exhaustive()
    }
}

impl Own {
    fn new(authority: Authority, addresses: Vec<Ipv4Addr>, today: i64) -> Result<Own, String> {
        let issued = authority.issue(addresses, today)?;
        Ok(Own {
            authority,
            issued: Mutex::new(issued),
        })
    }

    fn issued(&self) -> MutexGuard<'_, Issued> {
        self.issued.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Issues again when `addresses` aren't the certificate's or `today` is its day to be. One
    /// that can't be issued keeps the certificate there is.
    fn refresh(&self, issued: &mut Issued, addresses: Vec<Ipv4Addr>, today: i64) {
        if (issued.addresses != addresses || today >= issued.renew)
            && let Ok(new) = self.authority.issue(addresses, today)
        {
            *issued = new;
        }
    }
}

impl ResolvesServerCert for Own {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let mut issued = self.issued();
        if issued.looked.elapsed() >= LOOK_EVERY {
            issued.looked = Instant::now();
            self.refresh(&mut issued, private_addresses(), today());
        }
        Some(issued.certified.clone())
    }
}

/// The person's certificate chain and private key, both PEM. An Err names the file and what is
/// wrong with it.
pub fn files(cert: &Path, key: &Path) -> Result<Certificate, String> {
    let wrong = |file: &Path, why: &dyn std::fmt::Display| format!("{}: {why}", file.display());
    let chain = CertificateDer::pem_file_iter(cert)
        .map_err(|e| wrong(cert, &e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| wrong(cert, &e))?;
    if chain.is_empty() {
        return Err(format!("{} has no certificate", cert.display()));
    }
    let key = PrivateKeyDer::from_pem_file(key).map_err(|e| match e {
        PemError::NoItemsFound => format!("{} has no private key", key.display()),
        e => wrong(key, &e),
    })?;
    Ok(Certificate { chain, key })
}

/// The SHA-256 of a certificate's DER as 32 uppercase hex pairs joined by `:`, as Node's
/// `fingerprint256` gives it: what a client pins.
pub fn fingerprint(cert: &CertificateDer) -> String {
    let pairs: Vec<String> = digest(&SHA256, cert.as_ref())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();
    pairs.join(":")
}

/// What a connection code pins: the fingerprint as 64 lowercase hex characters, no colons.
pub fn pin(cert: &CertificateDer) -> String {
    fingerprint(cert).replace(':', "").to_lowercase()
}

/// ring's crypto.
fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Safe protocol versions (1.2 and 1.3), no client certificate.
fn builder() -> Result<rustls::ConfigBuilder<rustls::ServerConfig, WantsServerCert>, String> {
    rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map(|builder| builder.with_no_client_auth())
        .map_err(|e| e.to_string())
}

fn http_1_1(mut config: rustls::ServerConfig) -> Arc<rustls::ServerConfig> {
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

/// The server's TLS settings with the person's certificate, HTTP/1.1.
pub fn server_config(cert: Certificate) -> Result<Arc<rustls::ServerConfig>, String> {
    let config = builder()?
        .with_single_cert(cert.chain, cert.key)
        .map_err(|e| format!("The certificate and the key don't go together: {e}"))?;
    Ok(http_1_1(config))
}

/// The server's TLS settings with Aegis's authority, HTTP/1.1: the certificate it issues for this
/// computer's addresses, kept up with them (`Own`).
pub fn own_config(data: &Path) -> Result<Arc<rustls::ServerConfig>, String> {
    let own = Own::new(authority(data)?, private_addresses(), today())?;
    Ok(http_1_1(builder()?.with_cert_resolver(Arc::new(own))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_rustls::rustls::RootCertStore;
    use tokio_rustls::rustls::client::WebPkiServerVerifier;
    use tokio_rustls::rustls::client::danger::ServerCertVerifier;
    use tokio_rustls::rustls::pki_types::{ServerName, UnixTime};

    /// Whether a client that installed `root` takes `chain`'s certificate for `name`, now.
    fn trusted(root: &CertificateDer, chain: &[CertificateDer<'static>], name: &str) -> bool {
        let mut roots = RootCertStore::empty();
        roots.add(root.clone()).unwrap();
        let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider())
            .build()
            .unwrap();
        let name = ServerName::try_from(name.to_string()).unwrap();
        verifier
            .verify_server_cert(&chain[0], &chain[1..], &name, &[], UnixTime::now())
            .is_ok()
    }

    /// A certificate and key of the person's own, as files in `dir`.
    fn persons(dir: &Path) -> (PathBuf, PathBuf, CertificateDer<'static>) {
        let pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let cert = rcgen::CertificateParams::new(vec!["localhost".to_string()])
            .unwrap()
            .self_signed(&pair)
            .unwrap();
        let (cert_file, key_file) = (dir.join("c.pem"), dir.join("k.pem"));
        std::fs::write(&cert_file, cert.pem()).unwrap();
        std::fs::write(&key_file, pair.serialize_pem()).unwrap();
        (cert_file, key_file, cert.der().clone())
    }

    #[test]
    fn aegiss_authority_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let (cert, key) = root_paths(dir.path());
        let a = fingerprint(&root(dir.path()).unwrap());
        assert_eq!(fingerprint(&root(dir.path()).unwrap()), a);
        assert!(cert.exists() && key.exists());
        assert!(
            root_pem(dir.path())
                .unwrap()
                .starts_with("-----BEGIN CERTIFICATE-----")
        );
        assert_ne!(fingerprint(&renew(dir.path()).unwrap()), a);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&key).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // One file missing: both are made again.
        std::fs::remove_file(&key).unwrap();
        assert!(root(dir.path()).is_ok() && key.exists());
    }

    #[test]
    fn the_certificate_is_for_this_computer_and_issued_by_the_authority() {
        let dir = tempfile::tempdir().unwrap();
        let (lan, tailscale) = (Ipv4Addr::new(192, 168, 1, 5), Ipv4Addr::new(100, 90, 1, 2));
        let authority = authority(dir.path()).unwrap();
        let issued = authority.issue(vec![lan, tailscale], today()).unwrap();
        let chain = &issued.certified.cert;
        let root = root(dir.path()).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[1], root, "the authority comes after the certificate");
        for name in ["localhost", "127.0.0.1", "::1", "192.168.1.5", "100.90.1.2"] {
            assert!(trusted(&root, chain, name), "{name}");
        }
        assert!(
            !trusted(&root, chain, "192.168.1.6"),
            "an address it isn't for"
        );
        // Another authority's certificate isn't this one's.
        let other = tempfile::tempdir().unwrap();
        assert!(!trusted(
            &super::root(other.path()).unwrap(),
            chain,
            "localhost"
        ));
    }

    #[test]
    fn the_authority_vouches_only_for_private_addresses() {
        let dir = tempfile::tempdir().unwrap();
        let root = root(dir.path()).unwrap();
        let authority = authority(dir.path()).unwrap();
        // Issued for a public address, as a stolen key would: the name constraints refuse it, for
        // every name it holds.
        let public = authority
            .issue(vec![Ipv4Addr::new(8, 8, 8, 8)], today())
            .unwrap();
        for name in ["8.8.8.8", "localhost"] {
            assert!(!trusted(&root, &public.certified.cert, name), "{name}");
        }
        let lan = authority
            .issue(vec![Ipv4Addr::new(10, 0, 0, 7)], today())
            .unwrap();
        assert!(trusted(&root, &lan.certified.cert, "10.0.0.7"));
    }

    #[test]
    fn the_certificate_is_issued_again_when_an_address_moves_or_it_nears_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let (lan, tailscale) = (Ipv4Addr::new(192, 168, 1, 5), Ipv4Addr::new(100, 90, 1, 2));
        let day = today();
        let own = Own::new(authority(dir.path()).unwrap(), vec![lan], day).unwrap();
        let shown = |own: &Own| own.issued().certified.clone();
        let first = shown(&own);
        own.refresh(&mut own.issued(), vec![lan], day + 1);
        assert!(Arc::ptr_eq(&first, &shown(&own)), "nothing moved");

        own.refresh(&mut own.issued(), vec![lan, tailscale], day);
        let second = shown(&own);
        assert!(!Arc::ptr_eq(&first, &second), "Tailscale came");
        let root = root(dir.path()).unwrap();
        assert!(trusted(&root, &second.cert, "100.90.1.2"));
        own.refresh(&mut own.issued(), vec![tailscale], day);
        assert!(
            !trusted(&root, &shown(&own).cert, "192.168.1.5"),
            "the LAN went"
        );

        let renew = day - 1 + LEAF_DAYS - LEAF_RENEW_DAYS;
        let third = shown(&own);
        own.refresh(&mut own.issued(), vec![tailscale], renew - 1);
        assert!(Arc::ptr_eq(&third, &shown(&own)));
        own.refresh(&mut own.issued(), vec![tailscale], renew);
        assert!(!Arc::ptr_eq(&third, &shown(&own)), "near its end");
        assert_eq!(own.issued().renew, renew - 1 + LEAF_DAYS - LEAF_RENEW_DAYS);
    }

    #[test]
    fn only_private_addresses_are_issued_for() {
        for (address, private) in [
            ("10.1.2.3", true),
            ("172.16.0.1", true),
            ("172.31.255.255", true),
            ("172.32.0.1", false),
            ("192.168.0.10", true),
            ("100.64.0.1", true),
            ("100.127.255.254", true),
            ("100.63.255.255", false),
            ("100.128.0.1", false),
            ("8.8.8.8", false),
            ("169.254.1.1", false),
            ("127.0.0.1", false),
        ] {
            assert_eq!(is_private(address.parse().unwrap()), private, "{address}");
        }
        assert!(is_tailscale(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(!is_tailscale(Ipv4Addr::new(10, 0, 0, 1)));
        let found = private_addresses();
        let key = |address: &Ipv4Addr| (!is_tailscale(*address), *address);
        assert!(found.windows(2).all(|pair| key(&pair[0]) < key(&pair[1])));
        assert!(found.iter().all(|address| is_private(*address)));
    }

    #[test]
    fn the_fingerprint_is_the_sha256_of_the_der() {
        let der = CertificateDer::from(b"aegis".to_vec());
        let sum = digest(&SHA256, b"aegis");
        let want: Vec<String> = sum.as_ref().iter().map(|b| format!("{b:02X}")).collect();
        let got = fingerprint(&der);
        assert_eq!(got, want.join(":"));
        assert_eq!((got.len(), got.matches(':').count()), (95, 31));
        assert!(
            got.chars()
                .all(|c| c == ':' || c.is_ascii_digit() || ('A'..='F').contains(&c))
        );
    }

    #[test]
    fn the_pin_is_the_fingerprint_in_lowercase_hex() {
        let der = CertificateDer::from(b"aegis".to_vec());
        let pinned = pin(&der);
        assert_eq!(pinned, fingerprint(&der).replace(':', "").to_lowercase());
        assert!(
            pinned.len() == 64
                && pinned
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }

    #[test]
    fn dates_are_counted_from_the_epoch() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_723), (2024, 1, 1));
        assert_eq!(civil(19_782), (2024, 2, 29));
        assert_eq!(civil(-1), (1969, 12, 31));
    }

    #[test]
    fn the_persons_files_load_or_say_what_is_wrong() {
        let other = tempfile::tempdir().unwrap();
        let (cert, key, der) = persons(other.path());
        let loaded = files(&cert, &key).unwrap();
        assert_eq!(loaded.chain[0], der);
        let missing = other.path().join("nope.pem");
        assert!(
            files(&missing, &key)
                .err()
                .unwrap()
                .contains(missing.to_str().unwrap())
        );
        assert!(
            files(&cert, &missing)
                .err()
                .unwrap()
                .contains(missing.to_str().unwrap())
        );
        let junk = other.path().join("junk.pem");
        std::fs::write(&junk, "junk").unwrap();
        assert!(
            files(&junk, &key)
                .err()
                .unwrap()
                .contains("has no certificate")
        );
        assert!(
            files(&cert, &cert)
                .err()
                .unwrap()
                .contains("has no private key")
        );
        assert!(
            files(&cert, &junk)
                .err()
                .unwrap()
                .contains("has no private key")
        );
    }

    #[test]
    fn a_server_config_speaks_http_1_1() {
        let dir = tempfile::tempdir().unwrap();
        let config = own_config(dir.path()).unwrap();
        assert_eq!(config.alpn_protocols, vec![b"http/1.1".to_vec()]);
        let (cert, key, _) = persons(dir.path());
        let config = server_config(files(&cert, &key).unwrap()).unwrap();
        assert_eq!(config.alpn_protocols, vec![b"http/1.1".to_vec()]);
        // A key that isn't this certificate's is refused.
        let stranger = tempfile::tempdir().unwrap();
        let (_, other_key, _) = persons(stranger.path());
        let error = server_config(Certificate {
            chain: files(&cert, &key).unwrap().chain,
            key: files(&cert, &other_key).unwrap().key,
        })
        .err()
        .unwrap();
        assert!(error.contains("don't go together"), "{error}");
    }
}
