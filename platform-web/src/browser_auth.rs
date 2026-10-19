//! Browser-owned login forms and session cookies, independent of the renderer.
use crate::server::{credential, secure_headers, valid_origin};
use crate::{GatewayState, ShellBackendError};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, SET_COOKIE},
    },
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use rumahl_core::UserId;
use std::sync::Arc;
use zeroize::Zeroizing;

pub const SESSION_SECONDS: u64 = 12 * 60 * 60;

/// Public profile information shown on the OS sign-in screen.
pub struct LoginAccount {
    pub username: String,
    pub display_name: String,
}

/// Implementations persist and revoke both OS sessions and opaque credentials.
/// Secrets must never appear in Debug, logs, URLs or renderer payloads.
pub trait BrowserSessions: Send + Sync + 'static {
    fn login_accounts(&self) -> Result<Vec<LoginAccount>, ShellBackendError> {
        Ok(Vec::new())
    }

    fn login(
        &self,
        username: &str,
        password: String,
    ) -> Result<Zeroizing<String>, ShellBackendError>;
    /// Re-checks the account password without creating a new session. Used to
    /// gate security-sensitive changes such as the exposure mode.
    fn reauthenticate(&self, user_id: UserId, password: String) -> Result<(), ShellBackendError>;
    fn logout(&self, credential: &str) -> Result<(), ShellBackendError>;
}

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/login", get(login_page).post(login))
        .route("/login/appearance.js", get(login_appearance))
        .route("/login/wallpaper.jpg", get(login_wallpaper))
        .route("/logout", get(logout_page).post(logout))
        .layer(DefaultBodyLimit::max(8192))
}

async fn login_wallpaper() -> Response {
    let mut response = (
        [(CONTENT_TYPE, "image/jpeg")],
        include_bytes!("../assets/login-wallpaper.jpg").as_slice(),
    )
        .into_response();
    secure_headers(&mut response);
    response
}

// A blocking, same-origin script restores presentation before first paint.
// It has no access to account data and does not participate in authentication.
async fn login_appearance() -> Response {
    let mut response = (
        [(CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("login-appearance.js"),
    )
        .into_response();
    secure_headers(&mut response);
    response
}

pub(crate) fn login_redirect() -> Response {
    let mut response = Redirect::to("/login").into_response();
    secure_headers(&mut response);
    response
}

/// Same-host shell targets only; never bounce through API/auth/resource routes.
pub(crate) fn valid_shell_target(target: &str) -> bool {
    if target.len() > 2048
        || !target.starts_with('/')
        || target.starts_with("//")
        || target
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == '#' || c == ' ')
    {
        return false;
    }
    let Ok(url) = url::Url::parse(&format!("https://shell.invalid{target}")) else {
        return false;
    };
    let first = url.path().split('/').nth(1).unwrap_or("");
    !first.contains('%')
        && !first.starts_with('.')
        && !first.starts_with('@')
        && ![
            "api",
            "assets",
            "shell",
            "login",
            "logout",
            "recovery",
            "oauth2",
            "src",
            "node_modules",
            "favicon.ico",
            "robots.txt",
            "manifest.webmanifest",
            "service-worker.js",
            "os-client.js",
            "offline",
        ]
        .contains(&first)
}

pub(crate) fn login_redirect_to(target: &str) -> Response {
    if target == "/" || !valid_shell_target(target) {
        return login_redirect();
    }
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("next", target)
        .finish();
    let mut response = Redirect::to(&format!("/login?{query}")).into_response();
    secure_headers(&mut response);
    response
}

async fn login_page(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<std::collections::BTreeMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if state.browser_sessions.is_none() {
        return failure(StatusCode::SERVICE_UNAVAILABLE);
    }
    let next = query
        .get("next")
        .filter(|target| valid_shell_target(target))
        .map_or("/", String::as_str);
    let sessions = Arc::clone(state.browser_sessions.as_ref().unwrap());
    let accounts = match tokio::task::spawn_blocking(move || sessions.login_accounts()).await {
        Ok(Ok(accounts)) => accounts,
        _ => return failure(StatusCode::SERVICE_UNAVAILABLE),
    };
    let selected = query.get("user").map(String::as_str);
    render_page(&headers, false, false, next, &accounts, selected)
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
    let mut next = None;
    for (key, value) in url::form_urlencoded::parse(&body) {
        match key.as_ref() {
            "username" if username.is_none() => username = Some(value.into_owned()),
            "password" if password.is_none() => password = Some(Zeroizing::new(value.into_owned())),
            "next" if next.is_none() && valid_shell_target(&value) => {
                next = Some(value.into_owned())
            }
            _ => return failure(StatusCode::BAD_REQUEST),
        }
    }
    let next = next.as_deref().unwrap_or("/");
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
    let selected_username = username.clone();
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
            let mut response = if headers.get("accept").and_then(|value| value.to_str().ok())
                == Some("application/json")
            {
                axum::Json(serde_json::json!({ "next": next })).into_response()
            } else {
                Redirect::to(next).into_response()
            };
            response.headers_mut().insert(SET_COOKIE, cookie);
            secure_headers(&mut response);
            response
        }
        Ok(Err(ShellBackendError::Unauthorized)) => {
            let mut response =
                render_page(&headers, false, true, next, &[], Some(&selected_username));
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
    page_with_next(headers, logout, invalid, "/")
}

fn page_with_next(headers: &HeaderMap, logout: bool, invalid: bool, next: &str) -> Response {
    render_page(headers, logout, invalid, next, &[], Some(""))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
}

fn render_page(
    headers: &HeaderMap,
    logout: bool,
    invalid: bool,
    next: &str,
    accounts: &[LoginAccount],
    selected: Option<&str>,
) -> Response {
    let de = headers
        .get("accept-language")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|v| v.starts_with("de"));
    let (lang, signin, signout, choose, other, user, password, recovery, invalid_text) = if de {
        (
            "de",
            "Anmelden",
            "Abmelden",
            "Wer meldet sich an?",
            "Anderer Benutzer",
            "Benutzername",
            "Passwort",
            "Wiederherstellung",
            "Benutzername oder Passwort ist nicht korrekt. Bitte versuche es erneut.",
        )
    } else {
        (
            "en",
            "Sign in",
            "Sign out",
            "Who’s signing in?",
            "Other user",
            "Username",
            "Password",
            "Recovery",
            "The username or password is incorrect. Please try again.",
        )
    };
    let switch_user = if de {
        "Benutzer wechseln"
    } else {
        "Switch user"
    };
    let previous = if de {
        "Vorheriger Benutzer"
    } else {
        "Previous user"
    };
    let following = if de {
        "Nächster Benutzer"
    } else {
        "Next user"
    };
    let title = if logout { signout } else { signin };
    let query = |username: Option<&str>| {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("next", next);
        if let Some(username) = username {
            serializer.append_pair("user", username);
        }
        escape(&format!("/login?{}", serializer.finish()))
    };
    let content = if !logout && selected.is_none() && !accounts.is_empty() {
        let tiles = accounts.iter().map(|account| {
            let name = escape(&account.display_name);
            let initial = escape(&account.display_name.chars().next().unwrap_or('?').to_uppercase().to_string());
            let href = query(Some(&account.username));
            format!(r#"<a class="account" href="{href}"><span class="rumahl-avatar rumahl-avatar--lg" data-material="theme" aria-hidden="true"><span>{initial}</span></span><span>{name}</span><small>@{username}</small></a>"#, username = escape(&account.username))
        }).collect::<String>();
        format!(
            r#"<h1 class="visually-hidden">{choose}</h1><div class="account-slider"><nav id="login-accounts" class="accounts" aria-label="{choose}">{tiles}</nav><div class="slider-controls" hidden><button type="button" class="btn btn--secondary slider-step" data-step="-1" aria-label="{previous}" aria-controls="login-accounts"><svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m15 6-6 6 6 6" /></svg></button><span class="slider-position" aria-hidden="true"></span><button type="button" class="btn btn--secondary slider-step" data-step="1" aria-label="{following}" aria-controls="login-accounts"><svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m9 6 6 6-6 6" /></svg></button></div></div><a class="btn btn--secondary login-action" href="{other_url}">{other}</a>"#,
            other_url = query(Some(""))
        )
    } else {
        let username = selected.unwrap_or("");
        let account = accounts.iter().find(|account| account.username == username);
        let name = account.map_or(username, |account| account.display_name.as_str());
        let heading = if logout {
            signout
        } else if name.is_empty() {
            other
        } else {
            name
        };
        let initial = if name.is_empty() {
            "◎".to_owned()
        } else {
            name.chars().next().unwrap().to_uppercase().to_string()
        };
        let fields = if logout {
            String::new()
        } else {
            let username_field = if username.is_empty() || invalid {
                format!(
                    r#"<label><span class="visually-hidden">{user}</span><input placeholder="{user}" class="rumahl-input" name="username" value="{}" autocomplete="username" maxlength="128" required {}></label>"#,
                    escape(username),
                    if username.is_empty() { "autofocus" } else { "" }
                )
            } else {
                format!(
                    r#"<input type="hidden" name="username" value="{}">"#,
                    escape(username)
                )
            };
            format!(
                r#"{username_field}<div class="password-row"><label><span class="visually-hidden">{password}</span><input class="rumahl-input" name="password" type="password" placeholder="{password}" autocomplete="current-password" maxlength="1024" required {}></label><button class="btn btn--primary" type="submit" aria-label="{signin}" title="{signin}"><svg aria-hidden="true" focusable="false" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m9 6 6 6-6 6" /></svg></button></div><p class="login-status" role="status"></p>"#,
                if username.is_empty() { "" } else { "autofocus" }
            )
        };
        let submit = if logout {
            format!(r#"<button class="btn btn--primary" type="submit">{signout}</button>"#)
        } else {
            String::new()
        };
        let error = if invalid {
            format!(r#"<p class="error" role="alert">{invalid_text}</p>"#)
        } else {
            String::new()
        };
        let back = if logout {
            String::new()
        } else {
            format!(
                r#"<a class="btn btn--secondary login-action" href="{}"><svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="m15 6-6 6 6 6" /></svg>{switch_user}</a>"#,
                query(None)
            )
        };
        format!(
            r#"<div class="rumahl-avatar rumahl-avatar--lg selected-avatar" data-material="theme" aria-hidden="true"><span>{initial}</span></div><h1>{heading}</h1>{error}<form method="post" action="{action}" data-login-form><input type="hidden" name="next" value="{next}">{fields}{submit}</form>{back}"#,
            initial = escape(&initial),
            heading = escape(heading),
            action = if logout { "/logout" } else { "/login" },
            next = escape(next)
        )
    };
    let tokens = rumahl_ui_contracts::ResolvedTheme::stock().compile_css();
    // Never point at the shell re-export shims: they only contain an `@import`
    // a browser cannot resolve. The real stylesheets live in `@rumahl/ui`.
    let materials = include_str!("../../frontend/packages/shell/src/shell/materials.css");
    let controls = include_str!("../../frontend/packages/ui/src/components/controls.css");
    let classic = include_str!("../../frontend/packages/ui/src/classic.css");
    let css = include_str!("login.css");
    let html = format!(
        r#"<!doctype html><html lang="{lang}" class="shell" data-theme="com.rumahl.default" data-scheme="light"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title} · rumahl OS</title><style>{tokens}{materials}{controls}{classic}{css}</style><script src="/login/appearance.js"></script><script src="/os-client.js" defer></script></head><body><header class="lock-clock" aria-hidden="true"><span id="login-date"></span><time id="login-time"></time></header><main><section class="login-profiles" aria-label="{title}">{content}</section></main><footer><span class="brand">rumahl <span>OS</span></span><a href="/recovery">{recovery}</a></footer></body></html>"#
    );
    let mut response = Response::new(Body::from(html));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; script-src 'self'; worker-src 'self'; connect-src 'self'; img-src 'self'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"));
    secure_headers(&mut response);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn html(response: Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn account_picker_escapes_profiles_and_preserves_destination() {
        let accounts = vec![LoginAccount {
            username: "alice".into(),
            display_name: "<Alice & Bob>".into(),
        }];
        let body = html(render_page(
            &HeaderMap::new(),
            false,
            false,
            "/app/notes",
            &accounts,
            None,
        ))
        .await;
        assert!(body.contains("&lt;Alice &amp; Bob&gt;"));
        assert!(!body.contains("<Alice & Bob>"));
        assert!(body.contains("next=%2Fapp%2Fnotes&amp;user=alice"));
        assert!(body.contains("Other user"));
        assert!(!body.contains("name=\"password\""));
    }

    #[tokio::test]
    async fn selected_account_and_failed_login_keep_username_but_never_password() {
        for invalid in [false, true] {
            let body = html(render_page(
                &HeaderMap::new(),
                false,
                invalid,
                "/app/notes",
                &[],
                Some("alice"),
            ))
            .await;
            assert!(body.contains("name=\"username\" value=\"alice\""));
            assert!(body.contains("name=\"password\" type=\"password\""));
            assert!(body.contains("name=\"next\" value=\"/app/notes\""));
            assert_eq!(body.contains("<p class=\"error\" role=\"alert\""), invalid);
        }
    }

    #[tokio::test]
    async fn login_uses_shell_tokens_and_shared_controls_before_paint() {
        let response = page(&HeaderMap::new(), false, false);
        let policy = response.headers()[CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(policy.contains("script-src 'self'"));
        assert!(policy.contains("connect-src 'self'"));
        assert!(!policy.contains("script-src 'unsafe-inline'"));
        let body = html(response).await;
        assert!(body.contains("--rumahl-ui-color-accent:"));
        assert!(body.contains("class=\"rumahl-input\""));
        assert!(body.contains("class=\"btn btn--primary\""));
        assert!(body.contains("data-theme=\"com.rumahl.default\""));
        assert!(body.find("/login/appearance.js").unwrap() < body.find("</head>").unwrap());
        let script = login_appearance().await;
        assert_eq!(
            script.headers()[CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
    }

    #[test]
    fn return_targets_cannot_escape_shell_or_bypass_reserved_routes() {
        for target in [
            "/",
            "/app/test/documents?mode=launcher",
            "/settings/display",
        ] {
            assert!(valid_shell_target(target));
        }
        for target in [
            "//evil.test",
            "https://evil.test",
            "/\\evil.test",
            "/api/private",
            "/x/../api/private",
            "/%61pi/private",
            "/login",
            "/assets/missing.js",
            "/.well-known/openid-configuration",
            "/x\n",
        ] {
            assert!(!valid_shell_target(target), "{target}");
        }
        let response = login_redirect_to("/app/test?mode=launcher");
        assert_eq!(
            response.headers()["location"],
            "/login?next=%2Fapp%2Ftest%3Fmode%3Dlauncher"
        );
    }

    #[test]
    fn auth_forms_preserve_same_origin_for_csrf_validation() {
        for logout in [false, true] {
            let response = page(&HeaderMap::new(), logout, false);
            assert_eq!(response.headers()["referrer-policy"], "same-origin");
            assert_eq!(response.headers()["cache-control"], "private, no-store");
        }
    }
}
