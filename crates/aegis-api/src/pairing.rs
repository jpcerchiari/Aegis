//! The pairing page (docs/specs/api.md, Pair a phone): while Pair a phone is open in Aegis, a plain
//! HTTP page on the address it picked gives a phone Aegis's authority, a `.crt` for Android and a
//! `.mobileconfig` for an iPhone, with its fingerprint to compare with the one the dialog shows.
//! Plain HTTP because the phone can't check HTTPS before it trusts the authority: someone on the
//! network who swapped the file would show another fingerprint.

use crate::tls::{fingerprint, pin};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::digest::{SHA256, digest};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio_rustls::rustls::pki_types::CertificateDer;

/// How long a client has to send its request, and to take the answer.
const WAIT: Duration = Duration::from_secs(5);
/// The most of a request's head that is read.
const HEAD_MOST: usize = 8 * 1024;
/// How often the page looks for a connection, and whether it should stop.
const POLL: Duration = Duration::from_millis(50);

/// The pairing page while it runs. Dropping it stops it: its port closes within a moment. One
/// connection is answered at a time, and each is closed after its answer.
pub struct Page {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl Page {
    /// Starts the page on `ip`, at a port the system picks, for the authority `root` of the
    /// computer named `computer`.
    pub fn start(ip: IpAddr, root: &CertificateDer, computer: &str) -> Result<Page, String> {
        let listener = TcpListener::bind((ip, 0))
            .map_err(|e| format!("The pairing page can't listen on {ip}: {e}."))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        let files = Files::new(root, computer);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::Builder::new()
            .name("aegis-pairing".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => answer(stream, &files),
                        Err(_) => std::thread::sleep(POLL),
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Page { addr, stop })
    }

    /// Where the first QR code points: `http://<ip>:<port>/`.
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// What the page serves, made once.
struct Files {
    page: String,
    crt: Vec<u8>,
    profile: String,
}

/// One answer: its status line, its type, how to save it, and its body.
struct Answer<'a> {
    status: &'static str,
    kind: &'static str,
    saved_as: Option<&'static str>,
    body: &'a [u8],
}

const TEXT: &str = "text/plain; charset=utf-8";

impl Files {
    fn new(root: &CertificateDer, computer: &str) -> Files {
        Files {
            page: page(root, computer),
            crt: root.to_vec(),
            profile: mobileconfig(root, computer),
        }
    }

    /// The answer to `method` on `path`. A query is not looked at.
    fn answer<'a>(&'a self, method: &str, path: &str) -> Answer<'a> {
        let answer = |status: &'static str,
                      kind: &'static str,
                      saved_as: Option<&'static str>,
                      body: &'a [u8]| Answer {
            status,
            kind,
            saved_as,
            body,
        };
        if method != "GET" && method != "HEAD" {
            return answer("405 Method Not Allowed", TEXT, None, b"Only GET.\n");
        }
        match path.split('?').next().unwrap_or_default() {
            "/" => answer(
                "200 OK",
                "text/html; charset=utf-8",
                None,
                self.page.as_bytes(),
            ),
            "/aegis-ca.crt" => answer(
                "200 OK",
                "application/x-x509-ca-cert",
                Some("aegis-ca.crt"),
                &self.crt,
            ),
            "/aegis-ca.mobileconfig" => answer(
                "200 OK",
                "application/x-apple-aspen-config",
                Some("aegis-ca.mobileconfig"),
                self.profile.as_bytes(),
            ),
            _ => answer("404 Not Found", TEXT, None, b"Not here.\n"),
        }
    }
}

impl Answer<'_> {
    /// The status line and headers, with the API's own safety headers.
    fn head(&self) -> String {
        let saved = self.saved_as.map_or(String::new(), |name| {
            format!("Content-Disposition: attachment; filename=\"{name}\"\r\n")
        });
        format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{saved}Connection: close\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\n\
             Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; \
             frame-ancestors 'none'\r\n\r\n",
            self.status,
            self.kind,
            self.body.len()
        )
    }
}

/// Reads one request's head, answers it and closes the connection.
fn answer(mut stream: TcpStream, files: &Files) {
    // An accepted socket may keep the listener's non-blocking mode (Windows).
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(WAIT));
    let _ = stream.set_write_timeout(Some(WAIT));
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|end| end == b"\r\n\r\n") && head.len() < HEAD_MOST {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&chunk[..n]),
        }
    }
    let first = head.split(|byte| *byte == b'\n').next().unwrap_or_default();
    let line = String::from_utf8_lossy(first);
    let mut words = line.split_whitespace();
    let (method, path) = (
        words.next().unwrap_or_default(),
        words.next().unwrap_or_default(),
    );
    let answer = files.answer(method, path);
    let _ = stream.write_all(answer.head().as_bytes());
    if method != "HEAD" {
        let _ = stream.write_all(answer.body);
    }
    let _ = stream.flush();
}

/// Text for HTML and XML.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The page: the fingerprint to compare, the two files, and how to install each.
fn page(root: &CertificateDer, computer: &str) -> String {
    let (computer, fingerprint) = (escape(computer), fingerprint(root));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Aegis: trust this computer</title>
<style>
:root {{ color-scheme: dark; color: #f3edf9; background: #201a32; font: 16px/1.5 system-ui, sans-serif; }}
body {{ max-width: 34rem; margin: auto; padding: 1rem; }}
h1 {{ font-size: 1.4rem; }}
h2 {{ font-size: 1.15rem; margin-top: 1.6rem; }}
code {{ display: block; overflow-wrap: anywhere; font: 15px/1.6 ui-monospace, Menlo, Consolas, monospace; background: #2a2240; border: 1px solid #584b6d; border-radius: .45rem; padding: .6rem .75rem; }}
a {{ display: block; text-align: center; margin: .6rem 0; padding: .75rem; border-radius: .45rem; background: #f6d385; color: #241a2c; font-weight: 700; text-decoration: none; }}
li {{ margin-bottom: .4rem; }}
</style>
</head>
<body>
<h1>Trust Aegis on {computer}</h1>
<p>Install Aegis's authority on this phone, once. Its fingerprint must be the one Aegis shows beside the QR code:</p>
<code>{fingerprint}</code>
<p>If it isn't, don't install it: someone on this network may have swapped it.</p>
<h2>Android</h2>
<a href="/aegis-ca.crt">Download the certificate</a>
<ol>
<li>Open Settings ▸ Security ▸ Encryption and credentials ▸ Install a certificate ▸ CA certificate. The names vary between phones: search Settings for "CA certificate".</li>
<li>Choose Install anyway, then aegis-ca.crt in Downloads.</li>
</ol>
<h2>iPhone</h2>
<a href="/aegis-ca.mobileconfig">Download the profile</a>
<ol>
<li>Allow the download, then open Settings ▸ General ▸ VPN and Device Management ▸ Aegis on {computer} ▸ Install.</li>
<li>Then Settings ▸ General ▸ About ▸ Certificate Trust Settings: turn on Aegis local authority.</li>
</ol>
<p>Then scan the second QR code in Aegis.</p>
</body>
</html>
"#
    )
}

/// An iPhone's profile that installs `root` as a trusted root. Its identifiers come from the
/// authority's fingerprint, so downloading it again replaces the profile instead of adding one.
pub fn mobileconfig(root: &CertificateDer, computer: &str) -> String {
    let sum = digest(&SHA256, root.as_ref());
    let uuid = |half: usize| {
        let mut bytes: [u8; 16] = sum.as_ref()[half * 16..half * 16 + 16]
            .try_into()
            .unwrap_or_default();
        // Shaped as a version 4 UUID, which is what Apple's tools make.
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex: String = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
        format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        )
    };
    let (profile_uuid, root_uuid) = (uuid(0), uuid(1));
    let id = format!("io.github.jpcerchiari.aegis.authority.{}", &pin(root)[..16]);
    let (computer, data) = (escape(computer), STANDARD.encode(&root[..]));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>PayloadContent</key>
	<array>
		<dict>
			<key>PayloadCertificateFileName</key>
			<string>aegis-ca.crt</string>
			<key>PayloadContent</key>
			<data>{data}</data>
			<key>PayloadDescription</key>
			<string>Aegis's own authority on {computer}: it signs that computer's certificate.</string>
			<key>PayloadDisplayName</key>
			<string>Aegis local authority</string>
			<key>PayloadIdentifier</key>
			<string>{id}.root</string>
			<key>PayloadType</key>
			<string>com.apple.security.root</string>
			<key>PayloadUUID</key>
			<string>{root_uuid}</string>
			<key>PayloadVersion</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>PayloadDescription</key>
	<string>Lets this device open Aegis on {computer} over HTTPS.</string>
	<key>PayloadDisplayName</key>
	<string>Aegis on {computer}</string>
	<key>PayloadIdentifier</key>
	<string>{id}</string>
	<key>PayloadType</key>
	<string>Configuration</string>
	<key>PayloadUUID</key>
	<string>{profile_uuid}</string>
	<key>PayloadVersion</key>
	<integer>1</integer>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn root() -> (tempfile::TempDir, CertificateDer<'static>) {
        let dir = tempfile::tempdir().unwrap();
        let root = crate::tls::root(dir.path()).unwrap();
        (dir, root)
    }

    #[test]
    fn the_profile_installs_the_authority_as_a_root() {
        let (_dir, root) = root();
        let profile = mobileconfig(&root, "winx <&> desktop");
        assert!(profile.contains("<string>com.apple.security.root</string>"));
        assert!(profile.contains("<string>Configuration</string>"));
        assert!(profile.contains(&format!("<data>{}</data>", STANDARD.encode(&root[..]))));
        assert!(profile.contains("Aegis on winx &lt;&amp;&gt; desktop"));
        assert!(!profile.contains("<&>"));
        // The same authority makes the same profile; its two UUIDs differ and are well formed.
        assert_eq!(mobileconfig(&root, "winx <&> desktop"), profile);
        let uuids: Vec<&str> = profile
            .split("<key>PayloadUUID</key>")
            .skip(1)
            .filter_map(|rest| rest.split("<string>").nth(1)?.split("</string>").next())
            .collect();
        assert_eq!(uuids.len(), 2);
        assert_ne!(uuids[0], uuids[1]);
        for uuid in uuids {
            let parts: Vec<usize> = uuid.split('-').map(str::len).collect();
            assert_eq!(parts, [8, 4, 4, 4, 12], "{uuid}");
            assert_eq!(&uuid[14..15], "4", "{uuid}");
        }
    }

    #[test]
    fn the_page_shows_the_fingerprint_and_gives_both_files() {
        let (_dir, root) = root();
        let files = Files::new(&root, "winx");
        let home = files.answer("GET", "/");
        assert_eq!(home.status, "200 OK");
        let text = String::from_utf8_lossy(home.body);
        assert!(text.contains(&fingerprint(&root)));
        assert!(
            text.contains("href=\"/aegis-ca.crt\"")
                && text.contains("href=\"/aegis-ca.mobileconfig\"")
        );
        let crt = files.answer("GET", "/aegis-ca.crt");
        assert_eq!(crt.kind, "application/x-x509-ca-cert");
        assert_eq!(crt.body, &root[..]);
        assert!(crt.head().contains("filename=\"aegis-ca.crt\""));
        let profile = files.answer("HEAD", "/aegis-ca.mobileconfig");
        assert_eq!(profile.kind, "application/x-apple-aspen-config");
        assert_eq!(files.answer("GET", "/?from=qr").status, "200 OK");
        assert_eq!(files.answer("GET", "/api.json").status, "404 Not Found");
        assert_eq!(files.answer("POST", "/").status, "405 Method Not Allowed");
        let head = home.head();
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n") && head.ends_with("\r\n\r\n"));
        assert!(head.contains(&format!("Content-Length: {}\r\n", home.body.len())));
        assert!(head.contains("X-Content-Type-Options: nosniff"));
    }

    #[test]
    fn the_page_serves_until_dropped() {
        let (_dir, root) = root();
        let page = Page::start(IpAddr::V4(Ipv4Addr::LOCALHOST), &root, "winx").unwrap();
        let url = page.url();
        assert!(
            url.starts_with("http://127.0.0.1:") && url.ends_with('/'),
            "{url}"
        );
        let addr = page.addr;
        let get = |path: &str| -> std::io::Result<Vec<u8>> {
            let mut stream = TcpStream::connect(addr)?;
            stream.set_read_timeout(Some(Duration::from_secs(10)))?;
            write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n")?;
            let mut answer = Vec::new();
            stream.read_to_end(&mut answer)?;
            Ok(answer)
        };
        let answer = get("/aegis-ca.crt").unwrap();
        assert!(answer.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(answer.ends_with(&root[..]));
        drop(page);
        let start = std::time::Instant::now();
        while TcpStream::connect(addr).is_ok() {
            assert!(start.elapsed() < Duration::from_secs(5), "still listening");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn names_are_escaped() {
        assert_eq!(escape(r#"a&b<c>"d'"#), "a&amp;b&lt;c&gt;&quot;d&#39;");
    }
}
