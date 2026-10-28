//! Public offline resources. Never contain account, session or workspace data.
use crate::GatewayState;
use axum::{
    Router,
    body::Body,
    http::{
        HeaderValue,
        header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE},
    },
    response::Response,
    routing::get,
};
use std::sync::Arc;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/service-worker.js", get(worker))
        .route("/os-client.js", get(client))
        .route("/offline", get(page))
        .route("/splash", get(splash))
        .route("/manifest.webmanifest", get(manifest))
        .route("/icon.svg", get(icon_svg))
        .route("/icon-512.png", get(icon_png))
        .route("/rre-i18n.js", get(i18n))
        .route("/offline/wallpaper.jpg", get(wallpaper))
        .route(
            "/offline/ping",
            get(|| async { public(Body::empty(), "text/plain", false) }),
        )
}
fn public(body: impl Into<Body>, mime: &'static str, cache: bool) -> Response {
    let mut response = Response::new(body.into());
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(mime));
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if cache {
            "public, max-age=0, must-revalidate"
        } else {
            "no-store"
        }),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; script-src 'self'; worker-src 'self'; connect-src 'self'; img-src 'self'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"));
    response
}
async fn worker() -> Response {
    let mut response = public(
        include_str!("offline/service-worker.js"),
        "text/javascript; charset=utf-8",
        false,
    );
    // A service worker's fetches obey its script CSP. The neutral connectivity
    // probes are the only off-origin requests allowed, so the worker can tell
    // "this device is offline" apart from "rumahl OS is unreachable".
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; connect-src 'self' https://connectivitycheck.gstatic.com https://cp.cloudflare.com;",
        ),
    );
    response
}
async fn client() -> Response {
    public(
        include_str!("offline/os-client.js"),
        "text/javascript; charset=utf-8",
        false,
    )
}
/// Extensible recovery locale bundle. English is the source language; other
/// languages (currently German) are additional entries. Shared by the standalone
/// recovery page and the offline kit so both stay in step.
async fn i18n() -> Response {
    public(
        format!("window.RRE_I18N={};", include_str!("rre-i18n.json")),
        "text/javascript; charset=utf-8",
        true,
    )
}

/// Boot splash for slow navigations. Self-contained so the service worker can
/// serve it while the platform is still loading the shell.
async fn splash() -> Response {
    let body = include_str!("../../frontend/packages/shell/splash.html")
        .replace(" nonce=\"{{nonce}}\"", "");
    let mut response = public(body, "text/html; charset=utf-8", true);
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"),
    );
    response
}

async fn manifest() -> Response {
    public(
        include_str!("../../frontend/packages/shell/public/manifest.webmanifest"),
        "application/manifest+json",
        true,
    )
}

async fn icon_svg() -> Response {
    public(
        include_bytes!("../../frontend/packages/shell/public/icon.svg").as_slice(),
        "image/svg+xml",
        true,
    )
}

async fn icon_png() -> Response {
    public(
        include_bytes!("../../frontend/packages/shell/public/icon-512.png").as_slice(),
        "image/png",
        true,
    )
}

async fn page() -> Response {
    // Self-contained (inline script): it must render even when the platform is
    // unreachable and no other asset can be fetched.
    let body =
        include_str!("offline/index.html").replace("{{i18n}}", include_str!("rre-i18n.json"));
    let mut response = public(body, "text/html; charset=utf-8", true);
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"),
    );
    response
}
async fn wallpaper() -> Response {
    public(
        include_bytes!("../assets/login-wallpaper.jpg").as_slice(),
        "image/jpeg",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn offline_resources_are_public_but_worker_is_never_http_cached() {
        assert_eq!(worker().await.headers()[CACHE_CONTROL], "no-store");
        assert!(
            page().await.headers()[CACHE_CONTROL]
                .to_str()
                .unwrap()
                .starts_with("public")
        );
        assert_eq!(wallpaper().await.headers()[CONTENT_TYPE], "image/jpeg");
        assert_eq!(page().await.status(), axum::http::StatusCode::OK);
        assert!(!page().await.headers().contains_key("set-cookie"));
        assert_eq!(splash().await.status(), axum::http::StatusCode::OK);
        assert_eq!(manifest().await.status(), axum::http::StatusCode::OK);
        assert_eq!(icon_svg().await.status(), axum::http::StatusCode::OK);
        assert_eq!(icon_png().await.status(), axum::http::StatusCode::OK);
        assert!(
            splash().await.headers()[CACHE_CONTROL]
                .to_str()
                .unwrap()
                .starts_with("public")
        );
    }
}
