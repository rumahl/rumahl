//! Authenticated, same-origin fetch bridge for user-configured app store
//! sources. The shell cannot fetch arbitrary HTTPS origins directly because
//! its CSP is `connect-src 'self'`; this endpoint keeps the source URL out of
//! the browser network stack while retaining the normal shell authentication.
//!
//! Certificates that the system does not trust are presented to the user for an
//! explicit decision, like a browser. A trusted certificate is pinned by its
//! SHA-256 fingerprint for the current OS account and host; a changed
//! certificate triggers the prompt again.
use crate::server::{authenticate, credential, error, secure_headers, valid_origin};
use crate::{GatewayState, ShellIdentity};
use axum::{
    Router,
    body::Body,
    extract::{Query, State},
    http::{
        HeaderMap, StatusCode,
        header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

const MAX_STORE_BYTES: usize = 32 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const IMAGE_TYPES: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/gif",
    "image/avif",
];
const TRUST_FILE: &str = "app-store-trust.json";
const TRUST_FORMAT_VERSION: u32 = 1;
const MAX_TRUST_ENTRIES: usize = 512;
const MAX_TRUST_FILE_BYTES: u64 = 256 * 1024;
const TLS_TIMEOUT: Duration = Duration::from_secs(10);
/// Serializes trust-file writes in this process.
static TRUST_WRITE: Mutex<()> = Mutex::new(());

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/shell/store/fetch", get(fetch))
        .route("/api/v1/shell/store/image", get(image))
        .route(
            "/api/v1/shell/store/trust",
            post(trust).layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FetchQuery {
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TrustRequest {
    host: String,
    fingerprint: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TrustDocument {
    format_version: u32,
    entries: Vec<TrustEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TrustEntry {
    user_id: String,
    host: String,
    /// Lowercase hex SHA-256 of the DER certificate.
    fingerprint: String,
}

#[derive(Debug)]
struct CertificateInfo {
    host: String,
    fingerprint: String,
}

/// Validate the source endpoint. Store sources are HTTPS-only and never carry
/// credentials or fragments. Redirects are disabled at the client so a source
/// cannot silently forward the authenticated fetch to another origin.
fn source_url(value: &str) -> Option<Url> {
    if value.is_empty() || value.len() > 2048 {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url)
}

fn fingerprint(certificate: &[u8]) -> String {
    Sha256::digest(certificate)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn parse_fingerprint(value: &str) -> Option<String> {
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}

fn trust_host(value: &str) -> Option<String> {
    let host = value.trim().to_ascii_lowercase();
    (!host.is_empty()
        && host.len() <= 253
        && !host.bytes().any(|byte| {
            byte.is_ascii_whitespace() || matches!(byte, b'/' | b'\\' | b'@' | b'#' | b'?')
        }))
    .then_some(host)
}

fn trust_path(state_dir: &Path) -> PathBuf {
    state_dir.join(TRUST_FILE)
}

fn load_trust(path: &Path) -> Result<TrustDocument, ()> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TrustDocument {
                format_version: TRUST_FORMAT_VERSION,
                entries: Vec::new(),
            });
        }
        Err(_) => return Err(()),
    };
    if !metadata.is_file() || metadata.len() > MAX_TRUST_FILE_BYTES {
        return Err(());
    }
    let bytes = fs::read(path).map_err(|_| ())?;
    let document: TrustDocument = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if document.format_version != TRUST_FORMAT_VERSION
        || document.entries.len() > MAX_TRUST_ENTRIES
        || document.entries.iter().any(|entry| {
            entry.user_id.is_empty()
                || trust_host(&entry.host).as_deref() != Some(entry.host.as_str())
                || parse_fingerprint(&entry.fingerprint).as_deref()
                    != Some(entry.fingerprint.as_str())
        })
    {
        return Err(());
    }
    Ok(document)
}

fn save_trust(path: &Path, document: &TrustDocument) -> Result<(), ()> {
    let bytes = serde_json::to_vec_pretty(document).map_err(|_| ())?;
    if bytes.len() as u64 > MAX_TRUST_FILE_BYTES {
        return Err(());
    }
    let temporary = path.with_file_name(format!(
        ".{TRUST_FILE}.{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(());
    }
    Ok(())
}

fn trusted_fingerprint(state_dir: Option<&Path>, user_id: &str, host: &str) -> Option<String> {
    let path = trust_path(state_dir?);
    load_trust(&path)
        .ok()?
        .entries
        .into_iter()
        .find(|entry| entry.user_id == user_id && entry.host == host)
        .map(|entry| entry.fingerprint)
}

fn remember_certificate(
    state_dir: &Path,
    user_id: &str,
    host: &str,
    fingerprint: &str,
) -> Result<(), ()> {
    let _guard = TRUST_WRITE.lock().map_err(|_| ())?;
    let path = trust_path(state_dir);
    let mut document = load_trust(&path)?;
    document
        .entries
        .retain(|entry| !(entry.user_id == user_id && entry.host == host));
    if document.entries.len() >= MAX_TRUST_ENTRIES {
        document.entries.remove(0);
    }
    document.entries.push(TrustEntry {
        user_id: user_id.to_owned(),
        host: host.to_owned(),
        fingerprint: fingerprint.to_owned(),
    });
    save_trust(&path, &document)
}

/// Accepts any certificate. Used only for a short probing handshake so the
/// certificate can be shown to the user for an explicit decision.
#[derive(Debug)]
struct AcceptAnyCertificate;

impl ServerCertVerifier for AcceptAnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Accepts exactly one pinned leaf certificate. Signature handshakes are still
/// verified; only chain validation is replaced by the explicit user decision.
#[derive(Debug)]
struct PinnedCertificate {
    fingerprint: String,
}

impl ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity.as_ref()) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "store certificate does not match the trusted certificate".to_owned(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn tls_config(verifier: Arc<dyn ServerCertVerifier>) -> rustls::ClientConfig {
    let mut config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    // The store bridge deliberately uses HTTP/1.1.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config
}

fn trusted_tls_config() -> Result<rustls::ClientConfig, ()> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

async fn probe_certificate(
    host: &str,
    port: u16,
    config: rustls::ClientConfig,
) -> Result<Vec<u8>, ()> {
    let host_name = ServerName::try_from(host.to_owned()).map_err(|_| ())?;
    let tcp = tokio::time::timeout(TLS_TIMEOUT, tokio::net::TcpStream::connect((host, port)))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let stream = tokio::time::timeout(TLS_TIMEOUT, connector.connect(host_name, tcp))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    stream
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certificates| certificates.first())
        .map(|certificate| certificate.as_ref().to_vec())
        .ok_or(())
}

async fn untrusted_certificate(
    host: &str,
    port: u16,
    pinned: Option<&str>,
) -> Option<CertificateInfo> {
    if pinned.is_none()
        && let Ok(config) = trusted_tls_config()
        && probe_certificate(host, port, config).await.is_ok()
    {
        // The certificate is system-trusted, so the failure was not caused by
        // certificate validation. Do not show a misleading trust prompt.
        return None;
    }
    let config = tls_config(Arc::new(AcceptAnyCertificate));
    let certificate = probe_certificate(host, port, config).await.ok()?;
    let fingerprint = fingerprint(&certificate);
    if pinned == Some(fingerprint.as_str()) {
        return None;
    }
    Some(CertificateInfo {
        host: host.to_owned(),
        fingerprint,
    })
}

fn certificate_required(info: &CertificateInfo) -> Response {
    let mut response = axum::Json(serde_json::json!({
        "storeError": "untrusted-certificate",
        "host": info.host,
        "fingerprint": info.fingerprint,
        "algorithm": "sha256",
    }))
    .into_response();
    *response.status_mut() = StatusCode::PRECONDITION_REQUIRED;
    secure_headers(&mut response);
    response
}

fn store_client(pinned: Option<&str>) -> Result<reqwest::Client, ()> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("rumahl-store/1");
    if let Some(pinned) = pinned {
        builder = builder.use_preconfigured_tls(tls_config(Arc::new(PinnedCertificate {
            fingerprint: pinned.to_owned(),
        })));
    }
    builder.build().map_err(|_| ())
}

struct RemoteResponse {
    bytes: Vec<u8>,
    content_type: Option<String>,
}

enum RemoteFailure {
    Certificate(CertificateInfo),
    Unavailable,
    BadGateway,
    TooLarge,
}

fn remote_error(failure: RemoteFailure) -> Response {
    match failure {
        RemoteFailure::Certificate(info) => certificate_required(&info),
        RemoteFailure::Unavailable => error(StatusCode::SERVICE_UNAVAILABLE),
        RemoteFailure::BadGateway => error(StatusCode::BAD_GATEWAY),
        RemoteFailure::TooLarge => error(StatusCode::PAYLOAD_TOO_LARGE),
    }
}

async fn fetch_remote(
    state: &Arc<GatewayState>,
    identity: &ShellIdentity,
    target: &Url,
    accept: &str,
    maximum: usize,
) -> Result<RemoteResponse, RemoteFailure> {
    let Some(host) = target.host_str().map(str::to_owned) else {
        return Err(RemoteFailure::BadGateway);
    };
    let Some(port) = target.port_or_known_default() else {
        return Err(RemoteFailure::BadGateway);
    };
    let user_id = identity.user_id.to_string();
    let pinned = trusted_fingerprint(state.state_dir.as_deref(), &user_id, &host);
    let client = store_client(pinned.as_deref()).map_err(|_| RemoteFailure::Unavailable)?;

    let response = match client
        .get(target.clone())
        .header(ACCEPT, accept)
        .header(reqwest::header::COOKIE, "")
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            if let Some(info) = untrusted_certificate(&host, port, pinned.as_deref()).await {
                return Err(RemoteFailure::Certificate(info));
            }
            return Err(RemoteFailure::Unavailable);
        }
    };
    if response.status() != reqwest::StatusCode::OK {
        return Err(RemoteFailure::BadGateway);
    }
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(RemoteFailure::TooLarge);
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = match response.bytes().await {
        Ok(bytes) if bytes.len() <= maximum => bytes.to_vec(),
        Ok(_) => return Err(RemoteFailure::TooLarge),
        Err(_) => return Err(RemoteFailure::Unavailable),
    };
    Ok(RemoteResponse {
        bytes,
        content_type,
    })
}

async fn fetch(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<FetchQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(target) = source_url(&query.url) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let token = match credential(&headers) {
        Ok(token) => token,
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let identity = match authenticate(&state, &token).await {
        Ok(identity) => identity,
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let remote = match fetch_remote(
        &state,
        &identity,
        &target,
        "application/json",
        MAX_STORE_BYTES,
    )
    .await
    {
        Ok(remote) => remote,
        Err(failure) => return remote_error(failure),
    };

    let mut response = Response::new(Body::from(remote.bytes));
    response.headers_mut().insert(
        CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    secure_headers(&mut response);
    response
}

async fn image(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<FetchQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(target) = source_url(&query.url) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let token = match credential(&headers) {
        Ok(token) => token,
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let identity = match authenticate(&state, &token).await {
        Ok(identity) => identity,
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let remote = match fetch_remote(
        &state,
        &identity,
        &target,
        "image/avif,image/webp,image/png,image/jpeg,image/gif",
        MAX_IMAGE_BYTES,
    )
    .await
    {
        Ok(remote) => remote,
        Err(failure) => return remote_error(failure),
    };
    let Some(content_type) = remote.content_type.filter(|value| {
        IMAGE_TYPES
            .iter()
            .any(|allowed| value.eq_ignore_ascii_case(allowed))
    }) else {
        return error(StatusCode::BAD_GATEWAY);
    };
    let Ok(content_type) = axum::http::HeaderValue::from_str(&content_type) else {
        return error(StatusCode::BAD_GATEWAY);
    };
    let mut response = Response::new(Body::from(remote.bytes));
    response.headers_mut().insert(CONTENT_TYPE, content_type);
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, max-age=300"),
    );
    secure_headers(&mut response);
    response
}

async fn trust(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    if headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let token = match credential(&headers) {
        Ok(token) => token,
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let identity = match authenticate(&state, &token).await {
        Ok(identity) => identity,
        Err(_) => return error(StatusCode::UNAUTHORIZED),
    };
    let Ok(request) = serde_json::from_slice::<TrustRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(host) = trust_host(&request.host) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(fingerprint) = parse_fingerprint(&request.fingerprint) else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(state_dir) = state.state_dir.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let user_id = identity.user_id.to_string();
    let saved = tokio::task::spawn_blocking(move || {
        remember_certificate(&state_dir, &user_id, &host, &fingerprint)
    })
    .await;
    match saved {
        Ok(Ok(())) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            secure_headers(&mut response);
            response
        }
        _ => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        fingerprint, parse_fingerprint, remember_certificate, source_url, trusted_fingerprint,
        untrusted_certificate,
    };
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use std::sync::Arc;

    const TEST_CERTIFICATE: &[u8] = include_bytes!("../tests/fixtures/store-test-cert.der");
    const TEST_KEY: &[u8] = include_bytes!("../tests/fixtures/store-test-key.der");

    async fn tls_server() -> u16 {
        let certificate = CertificateDer::from(TEST_CERTIFICATE.to_vec());
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(TEST_KEY.to_vec()));
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            for _ in 0..8 {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    if let Ok(mut stream) = acceptor.accept(stream).await {
                        use tokio::io::AsyncWriteExt;
                        let _ = stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                            .await;
                    }
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn probes_untrusted_certificates_and_accepts_an_explicit_pin() {
        let port = tls_server().await;
        let expected = fingerprint(TEST_CERTIFICATE);
        let info = untrusted_certificate("127.0.0.1", port, None)
            .await
            .expect("self-signed certificate should require a decision");
        assert_eq!(info.host, "127.0.0.1");
        assert_eq!(info.fingerprint, expected);
        assert!(
            untrusted_certificate("127.0.0.1", port, Some(&expected))
                .await
                .is_none(),
            "a matching pin must be accepted"
        );
        assert!(
            untrusted_certificate("127.0.0.1", port, Some(&"0".repeat(64)))
                .await
                .is_some(),
            "a changed certificate must trigger a new decision"
        );
    }

    #[test]
    fn accepts_only_credential_free_https_urls() {
        assert!(source_url("https://store.example/index.json").is_some());
        assert!(source_url("http://store.example/index.json").is_none());
        assert!(source_url("https://user:pass@store.example/index.json").is_none());
        assert!(source_url("https://store.example/index.json#fragment").is_none());
        assert!(source_url("file:///tmp/index.json").is_none());
    }

    #[test]
    fn fingerprints_are_lowercase_sha256_hex() {
        let value = fingerprint(b"certificate");
        assert_eq!(value.len(), 64);
        assert_eq!(parse_fingerprint(&value).as_deref(), Some(value.as_str()));
        assert!(parse_fingerprint(&value.to_ascii_uppercase()).is_some());
        assert!(parse_fingerprint("abcd").is_none());
        assert!(parse_fingerprint(&"z".repeat(64)).is_none());
    }

    #[test]
    fn trust_is_scoped_to_the_user_and_host_and_replaces_old_pins() {
        let directory = tempfile::tempdir().unwrap();
        remember_certificate(directory.path(), "user-a", "store.example", &"a".repeat(64)).unwrap();
        remember_certificate(directory.path(), "user-b", "store.example", &"b".repeat(64)).unwrap();
        assert_eq!(
            trusted_fingerprint(Some(directory.path()), "user-a", "store.example"),
            Some("a".repeat(64))
        );
        assert_eq!(
            trusted_fingerprint(Some(directory.path()), "user-b", "store.example"),
            Some("b".repeat(64))
        );
        assert_eq!(
            trusted_fingerprint(Some(directory.path()), "user-a", "other.example"),
            None
        );
        remember_certificate(directory.path(), "user-a", "store.example", &"c".repeat(64)).unwrap();
        assert_eq!(
            trusted_fingerprint(Some(directory.path()), "user-a", "store.example"),
            Some("c".repeat(64))
        );
    }
}
