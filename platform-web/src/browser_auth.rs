//! Browser-owned login forms and session cookies, independent of the renderer.
use crate::server::{credential, secure_headers, valid_origin};
use crate::{GatewayState, ShellBackendError};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, SET_COOKIE},
    },
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use std::sync::Arc;
use zeroize::Zeroizing;

pub const SESSION_SECONDS: u64 = 12 * 60 * 60;

/// Implementations persist and revoke both OS sessions and opaque credentials.
/// Secrets must never appear in Debug, logs, URLs or renderer payloads.
pub trait BrowserSessions: Send + Sync + 'static {
    fn login(
        &self,
        username: &str,
        password: String,
    ) -> Result<Zeroizing<String>, ShellBackendError>;
    fn logout(&self, credential: &str) -> Result<(), ShellBackendError>;
}

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/login", get(login_page).post(login))
        .route("/logout", get(logout_page).post(logout))
        .layer(DefaultBodyLimit::max(8192))
}

pub(crate) fn login_redirect() -> Response {
    let mut response = Redirect::to("/login").into_response();
    secure_headers(&mut response);
    response
}

async fn login_page(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if state.browser_sessions.is_none() {
        return failure(StatusCode::SERVICE_UNAVAILABLE);
    }
    page(&headers, false, false)
}

async fn logout_page(headers: HeaderMap) -> Response {
    page(&headers, true, false)
}

async fn login(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return failure(StatusCode::FORBIDDEN);
    }
    let Some(sessions) = &state.browser_sessions else {
        return failure(StatusCode::SERVICE_UNAVAILABLE);
    };
    if headers
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(|v| v.split(';').next().unwrap_or("").trim())
        != Some("application/x-www-form-urlencoded")
    {
        return failure(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let mut username = None;
    let mut password = None;
    for (key, value) in url::form_urlencoded::parse(&body) {
        match key.as_ref() {
            "username" if username.is_none() => username = Some(value.into_owned()),
            "password" if password.is_none() => password = Some(Zeroizing::new(value.into_owned())),
            _ => return failure(StatusCode::BAD_REQUEST),
        }
    }
    let (Some(username), Some(mut password)) = (username, password) else {
        return failure(StatusCode::BAD_REQUEST);
    };
    if username.len() > 128 {
        return failure(StatusCode::BAD_REQUEST);
    }
    // Bound expensive password hashes even when requests are canceled.
    let Ok(permit) = state.login_slots.clone().try_acquire_owned() else {
        return failure(StatusCode::TOO_MANY_REQUESTS);
    };
    let sessions = Arc::clone(sessions);
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        sessions.login(&username, std::mem::take(&mut *password))
    })
    .await;
    match result {
        Ok(Ok(token)) => {
            let cookie = Zeroizing::new(format!(
                "__Host-rumahl_session={}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={SESSION_SECONDS}",
                token.as_str()
            ));
            let Ok(mut cookie) = HeaderValue::from_str(&cookie) else {
                return failure(StatusCode::SERVICE_UNAVAILABLE);
            };
            cookie.set_sensitive(true);
            let mut response = Redirect::to("/").into_response();
            response.headers_mut().insert(SET_COOKIE, cookie);
            secure_headers(&mut response);
            response
        }
        Ok(Err(ShellBackendError::Unauthorized)) => {
            let mut response = page(&headers, false, true);
            *response.status_mut() = StatusCode::UNAUTHORIZED;
            response
        }
        _ => failure(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn logout(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if !valid_origin(&headers, &state.config.public_origin) {
        return failure(StatusCode::FORBIDDEN);
    }
    let Some(sessions) = &state.browser_sessions else {
        return failure(StatusCode::SERVICE_UNAVAILABLE);
    };
    if let Ok(token) = credential(&headers) {
        let sessions = Arc::clone(sessions);
        if !matches!(
            tokio::task::spawn_blocking(move || sessions.logout(&token)).await,
            Ok(Ok(()))
        ) {
            return failure(StatusCode::SERVICE_UNAVAILABLE);
        }
    }
    let mut response = login_redirect();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_static(
            "__Host-rumahl_session=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0",
        ),
    );
    response
}

fn failure(status: StatusCode) -> Response {
    let mut response = (status, "Request unavailable. / Anfrage nicht verfügbar.").into_response();
    secure_headers(&mut response);
    response
}

fn page(headers: &HeaderMap, logout: bool, invalid: bool) -> Response {
    let de = headers
        .get("accept-language")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|v| v.starts_with("de"));
    let (lang, title, user, password, invalid_text, recovery) = if de {
        (
            "de",
            if logout { "Abmelden" } else { "Anmelden" },
            "Benutzername",
            "Passwort",
            "Anmeldung fehlgeschlagen. Bitte versuche es erneut.",
            "Wiederherstellung",
        )
    } else {
        (
            "en",
            if logout { "Sign out" } else { "Sign in" },
            "Username",
            "Password",
            "Sign-in failed. Please try again.",
            "Recovery",
        )
    };
    let fields = if logout {
        String::new()
    } else {
        format!(
            r#"<label>{user}<input name="username" autocomplete="username" maxlength="128" required autofocus></label><label>{password}<input name="password" type="password" autocomplete="current-password" maxlength="1024" required></label>"#
        )
    };
    let error = if invalid {
        format!(r#"<p role="alert">{invalid_text}</p>"#)
    } else {
        String::new()
    };
    let action = if logout { "/logout" } else { "/login" };
    let html = format!(
        r#"<!doctype html><html lang="{lang}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title} · rumahl</title><style>html{{font:18px system-ui;background:#f4f5f7;color:#17202e}}main{{max-width:24rem;margin:12vh auto;padding:2rem;background:white;border-radius:1rem}}label,input,button{{display:block}}label{{margin:1rem 0}}input{{box-sizing:border-box;width:100%;padding:.7rem;font:inherit}}button{{padding:.7rem 1.2rem;font:inherit;cursor:pointer}}a{{display:inline-block;margin-top:1rem}}:focus-visible{{outline:3px solid #365ac4;outline-offset:3px}}</style><main><p>rumahl OS</p><h1>{title}</h1>{error}<form method="post" action="{action}">{fields}<button type="submit">{title}</button></form><a href="/recovery">{recovery}</a></main></html>"#
    );
    let mut response = Response::new(Body::from(html));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"));
    secure_headers(&mut response);
    response
}
