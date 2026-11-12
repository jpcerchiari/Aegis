//! The server over TLS (docs/specs/api.md, Encryption).

mod common;

use aegis_api::store::{Listen, Scope, Settings, Tls};
use aegis_api::{Options, tls};
use common::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use tokio_rustls::rustls::client::WebPkiServerVerifier;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::{self, CryptoProvider};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{
    self, ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme,
};

fn own() -> Settings {
    Settings {
        tls: Tls::Own,
        port: 0,
        ..Default::default()
    }
}

/// Records the chain a server presents, and checks its signatures, but trusts any.
#[derive(Debug)]
struct Recording {
    provider: Arc<CryptoProvider>,
    seen: Mutex<Option<Vec<CertificateDer<'static>>>>,
}

impl ServerCertVerifier for Recording {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let chain = std::iter::once(end_entity)
            .chain(intermediates)
            .map(|cert| cert.clone().into_owned())
            .collect();
        *self.seen.lock().unwrap() = Some(chain);
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// The chain the server on `port` shows, its certificate first, from a real handshake.
fn presented(port: u16) -> Vec<CertificateDer<'static>> {
    let provider = Arc::new(crypto::ring::default_provider());
    let verifier = Arc::new(Recording {
        provider: provider.clone(),
        seen: Mutex::new(None),
    });
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let mut connection =
        ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap())
            .unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let mut stream = rustls::Stream::new(&mut connection, &mut tcp);
    stream
        .write_all(b"GET /v1/nope HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    assert!(
        String::from_utf8_lossy(&answer).starts_with("HTTP/1.1 401"),
        "{}",
        String::from_utf8_lossy(&answer)
    );
    assert_eq!(connection.alpn_protocol(), Some(&b"http/1.1"[..]));
    let seen = verifier.seen.lock().unwrap().clone();
    seen.expect("a certificate was shown")
}

#[test]
fn https_serves_with_aegiss_certificate() {
    let (text, token) = token(Scope::Read, None);
    let fake = fake_with(own(), vec![token], |_| {});
    assert!(fake.base.starts_with("https://"));
    let answer = fake.call(
        req("GET", "/v1/state")
            .token(&text)
            .agent(https_unverified()),
    );
    assert_eq!(answer.status, 200);
    // No HSTS: a browser would pin the host for a year, and the certificate changes.
    assert_eq!(answer.header("strict-transport-security"), None);
    assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
    let refused = fake.call(req("GET", "/v1/state").agent(https_unverified()));
    assert_eq!(
        (refused.status, refused.header("strict-transport-security")),
        (401, None)
    );
    // Plain HTTP to the same port is no HTTP at all.
    let plain = format!("http://127.0.0.1:{}", fake.server.addr().port());
    let attempt = http().run(req("GET", "/v1/state").token(&text).build(&plain));
    assert!(attempt.is_err() || attempt.is_ok_and(|r| r.status() != 200));
    // And the handshake that failed was audited.
    let log = || {
        std::fs::read_to_string(aegis_api::store::folder(fake.dir.path()).join("noise.log"))
            .unwrap_or_default()
    };
    wait_for("tls_failed in the audit log", || {
        log().contains("\"tls_failed\"")
    });
}

#[test]
fn the_mobile_shell_is_public_only_over_tls_and_the_api_still_needs_a_token() {
    let secured = fake_with(own(), vec![], |_| {});
    let get = |path| secured.call(req("GET", path).agent(https_unverified()));
    let page = get("/mobile/");
    assert_eq!(page.status, 200);
    assert_eq!(
        page.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(page.header("cache-control"), Some("no-store"));
    let policy = page.header("content-security-policy").unwrap();
    assert!(policy.contains("script-src 'self'") && policy.contains("frame-ancestors 'none'"));
    assert!(page.text.contains("app.webmanifest") && page.text.contains("app.js"));
    let manifest = get("/mobile/app.webmanifest");
    assert_eq!(manifest.json["start_url"], "/mobile/");
    assert_eq!(manifest.json["display"], "standalone");
    for icon in manifest.json["icons"].as_array().unwrap() {
        let path = icon["src"].as_str().unwrap().to_string();
        let answer = secured.call(req("GET", &path).agent(https_unverified()));
        assert_eq!(answer.status, 200);
        assert_eq!(answer.header("content-type"), Some("image/png"));
    }
    for (path, mime) in [
        ("/mobile/app.js", "text/javascript; charset=utf-8"),
        ("/mobile/app.css", "text/css; charset=utf-8"),
    ] {
        let answer = get(path);
        assert_eq!(answer.status, 200);
        assert_eq!(answer.header("content-type"), Some(mime));
    }
    assert_eq!(get("/mobile/unknown").status, 401);
    assert_eq!(get("/v1/events").status, 401);
    assert_eq!(
        secured
            .call(req("POST", "/mobile/").agent(https_unverified()))
            .status,
        401
    );

    let plain = fake(vec![], |_| {});
    assert_eq!(plain.call(req("GET", "/mobile/")).status, 401);
}

#[test]
fn the_served_certificate_is_issued_by_the_authority_shown() {
    let fake = fake_with(own(), vec![], |_| {});
    let root = tls::root(fake.dir.path()).unwrap();
    let chain = presented(fake.server.addr().port());
    assert_eq!(chain.len(), 2, "the certificate, then the authority");
    assert_eq!(tls::fingerprint(&chain[1]), tls::fingerprint(&root));
    // What a phone that installed the authority checks: names, dates, the signature.
    let mut roots = rustls::RootCertStore::empty();
    roots.add(root).unwrap();
    let provider = Arc::new(crypto::ring::default_provider());
    let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
        .build()
        .unwrap();
    for name in ["localhost", "127.0.0.1", "::1"] {
        let name = ServerName::try_from(name).unwrap();
        let checked = verifier.verify_server_cert(&chain[0], &[], &name, &[], UnixTime::now());
        assert!(checked.is_ok(), "{name:?}: {checked:?}");
    }
}

#[test]
fn a_phone_trades_its_pairing_code_for_its_token() {
    let fake = fake_with(own(), vec![], |_| {});
    let pair = |code: &str| {
        fake.call(
            req("POST", "/v1/pair")
                .json(&serde_json::json!({ "code": code }))
                .agent(https_unverified()),
        )
    };
    let answer = pair("right");
    assert_eq!(answer.status, 200, "{}", answer.text);
    assert_eq!(answer.json["token"], "tpl_paired");
    let asked = fake.asked();
    let last = asked.last().unwrap();
    assert_eq!(
        (
            last.method.as_str(),
            last.terminal.as_str(),
            last.client.as_str()
        ),
        (aegis_api::PAIR, aegis_api::PAIRING, aegis_api::CLIENT)
    );
    assert_eq!(last.params["code"], "right");

    let wrong = pair("wrong");
    assert_eq!(wrong.status, 401);
    assert_eq!(wrong.json["error"]["code"], "auth");
    assert!(wrong.json.get("token").is_none());
    // No code, or not JSON: refused before the UI thread is asked.
    let before = fake.asked().len();
    let empty = fake.call(
        req("POST", "/v1/pair")
            .json(&serde_json::json!({}))
            .agent(https_unverified()),
    );
    assert_eq!(empty.status, 400);
    let text = fake.call(
        req("POST", "/v1/pair")
            .raw("text/plain", "right")
            .agent(https_unverified()),
    );
    assert_eq!(text.status, 415);
    assert_eq!(fake.asked().len(), before);

    // A code can't be guessed: wrong ones count against the address, and past the limit even the
    // right one waits.
    let mut status = 401;
    for _ in 0..20 {
        status = pair("wrong").status;
        if status == 429 {
            break;
        }
    }
    assert_eq!(status, 429);
    let limited = pair("right");
    assert_eq!(limited.status, 429);
    assert!(limited.header("retry-after").is_some());
}

#[test]
fn pairing_needs_https() {
    let fake = fake(vec![], |_| {});
    let answer = fake.call(req("POST", "/v1/pair").json(&serde_json::json!({ "code": "right" })));
    assert_eq!(answer.status, 401);
    assert!(
        fake.asked()
            .iter()
            .all(|seen| seen.method != aegis_api::PAIR)
    );
}

#[test]
fn the_persons_certificate_is_served() {
    let dir = tempfile::tempdir().unwrap();
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let cert = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .self_signed(&key)
        .unwrap();
    let (cert_file, key_file) = (dir.path().join("mine.pem"), dir.path().join("mine.key"));
    std::fs::write(&cert_file, cert.pem()).unwrap();
    std::fs::write(&key_file, key.serialize_pem()).unwrap();
    let settings = Settings {
        tls: Tls::Files {
            cert: cert_file.clone(),
            key: key_file.clone(),
        },
        port: 0,
        ..Default::default()
    };
    let fake = fake_with(settings.clone(), vec![], |_| {});
    let got = tls::fingerprint(&presented(fake.server.addr().port())[0]);
    assert_eq!(got, tls::fingerprint(cert.der()));
    // A key that doesn't load keeps it from starting, and the error names the file.
    std::fs::write(&key_file, "not a key").unwrap();
    let error = Options::from(&settings, dir.path()).err().unwrap();
    assert!(error.contains(key_file.to_str().unwrap()), "{error}");
    // Nor does a key that isn't this certificate's.
    let other = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    std::fs::write(&key_file, other.serialize_pem()).unwrap();
    assert!(Options::from(&settings, dir.path()).is_err());
}

#[test]
fn every_network_needs_https() {
    let (text, token) = token(Scope::Read, None);
    let settings = Settings {
        listen: Listen::All,
        tls: Tls::Own,
        port: 0,
        ..Default::default()
    };
    let fake = fake_with(settings, vec![token], |_| {});
    assert!(
        fake.server.addr().ip().is_unspecified(),
        "bound to every interface"
    );
    let answer = fake.call(
        req("GET", "/v1/state")
            .token(&text)
            .agent(https_unverified()),
    );
    assert_eq!(answer.status, 200);
}

#[test]
fn a_silent_client_does_not_hold_a_handshake_forever() {
    let fake = fake_with(own(), vec![], |_| {});
    // It connects and says nothing: the server keeps answering others meanwhile.
    let _quiet = TcpStream::connect(("127.0.0.1", fake.server.addr().port())).unwrap();
    let answer = fake.call(req("GET", "/v1/state").agent(https_unverified()));
    assert_eq!(answer.status, 401);
}
