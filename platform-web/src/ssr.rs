use std::path::Path;
use std::time::Duration;

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;

pub struct RenderedHtml {
    pub body: Bytes,
    pub csp: String,
}

pub async fn render(
    socket: &Path,
    snapshot_json: &str,
    nonce: &str,
    frame_origins: &[String],
) -> Result<RenderedHtml, ()> {
    tokio::time::timeout(Duration::from_secs(7), async {
        let stream = UnixStream::connect(socket).await.map_err(|_| ())?;
        let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| ())?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        let snapshot: serde_json::Value = serde_json::from_str(snapshot_json).map_err(|_| ())?;
        let payload = serde_json::json!({
            "snapshot": snapshot,
            "nonce": nonce,
            "frameOrigins": frame_origins,
        });
        let body = serde_json::to_vec(&payload).map_err(|_| ())?;
        if body.len() > 300 * 1024 {
            return Err(());
        }
        let request = Request::builder()
            .method("POST")
            .uri("/render")
            .header("host", "rumahl-shell.internal")
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body)))
            .map_err(|_| ())?;
        let response = sender.send_request(request).await.map_err(|_| ())?;
        if response.status() != StatusCode::OK
            || response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                != Some("text/html; charset=utf-8")
        {
            return Err(());
        }
        let csp = response
            .headers()
            .get("content-security-policy")
            .and_then(|v| v.to_str().ok())
            .ok_or(())?
            .to_owned();
        if !csp.contains("frame-ancestors 'none'") || !csp.contains(&format!("'nonce-{nonce}'")) {
            return Err(());
        }
        let body = Limited::new(response.into_body(), 1024 * 1024)
            .collect()
            .await
            .map_err(|_| ())?
            .to_bytes();
        Ok(RenderedHtml { body, csp })
    })
    .await
    .map_err(|_| ())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn sends_only_snapshot_nonce_and_frame_origins_to_private_renderer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ssr.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let mut nonce_bytes = [0_u8; 24];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce: String = nonce_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let expected_nonce = nonce.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buffer[..read]);
                let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                    continue;
                };
                let head = std::str::from_utf8(&request[..end]).unwrap();
                let length: usize = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse().ok())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("POST /render HTTP/1.1"));
            assert!(!request.to_ascii_lowercase().contains("cookie:"));
            assert!(!request.to_ascii_lowercase().contains("x-rumahl-user"));
            assert!(request.contains("\"frameOrigins\":[\"https://weather.apps.rumahl.dev\"]"));
            let payload: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(payload["nonce"].as_str(), Some(expected_nonce.as_str()));
            let body = "<html></html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-security-policy: default-src 'none'; script-src 'nonce-{expected_nonce}'; frame-ancestors 'none'\r\ncontent-length: {}\r\n\r\n{body}",
                body.len(),
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let rendered = render(
            &path,
            r#"{"snapshotVersion":1}"#,
            &nonce,
            &["https://weather.apps.rumahl.dev".to_owned()],
        )
        .await
        .unwrap();
        assert_eq!(rendered.body, "<html></html>");
        assert!(
            rendered
                .csp
                .contains(&format!("'nonce-{}'", nonce.as_str()))
        );
        server.await.unwrap();
    }
}
