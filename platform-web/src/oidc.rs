//! OIDC routes are optional; neither the Shell nor recovery depends on them.
//! The browser identity always comes from the OS session cookie, never a form.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::header::{
    AUTHORIZATION, CONTENT_SECURITY_POLICY, CONTENT_TYPE, LOCATION, WWW_AUTHENTICATE,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::Response;
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rumahl_core::{OidcScope, SessionId, UnixTimestamp, UserId};
use rumahl_oidc_provider::{
    ApprovedAuthorization, OidcAccessTokenStore, OidcAuthorizationStore, OidcClientRepository,
    OidcProtocol, OidcProtocolError, OidcTokenResponse, OidcUserSource,
};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::server::{GatewayState, authenticate, credential, secure_headers, valid_origin};

const MAX_FORM_BYTES: usize = 8192;

pub trait OidcGateway: Send + Sync + 'static {
    fn client_display_name(&self, client: &str) -> Result<String, OidcProtocolError>;
    #[allow(clippy::too_many_arguments)]
    fn begin(
        &self,
        client: &str,
        redirect: &str,
        scopes: Vec<OidcScope>,
        state: &str,
        nonce: &str,
        challenge: &str,
        user: UserId,
        session: SessionId,
        now: UnixTimestamp,
    ) -> Result<String, OidcProtocolError>;
    fn approve(
        &self,
        transaction: &str,
        client: &str,
        scopes: Vec<OidcScope>,
        user: UserId,
        session: SessionId,
        now: UnixTimestamp,
    ) -> Result<ApprovedAuthorization, OidcProtocolError>;
    fn exchange(
        &self,
        client: &str,
        secret: Option<&str>,
        code: &str,
        redirect: &str,
        verifier: &str,
        now: UnixTimestamp,
    ) -> Result<OidcTokenResponse, OidcProtocolError>;
    fn userinfo(&self, token: &str, now: UnixTimestamp) -> Result<Value, OidcProtocolError>;
    fn discovery(&self) -> Value;
    fn jwks(&self) -> Value;
}

impl<C, A, T, U> OidcGateway for OidcProtocol<C, A, T, U>
where
    C: OidcClientRepository + Send + Sync + 'static,
    A: OidcAuthorizationStore,
    T: OidcAccessTokenStore,
    U: OidcUserSource,
{
    fn client_display_name(&self, client: &str) -> Result<String, OidcProtocolError> {
        self.client_display_name(client)
    }
    fn begin(
        &self,
        client: &str,
        redirect: &str,
        scopes: Vec<OidcScope>,
        state: &str,
        nonce: &str,
        challenge: &str,
        user: UserId,
        session: SessionId,
        now: UnixTimestamp,
    ) -> Result<String, OidcProtocolError> {
        self.begin(
            client, redirect, scopes, state, nonce, challenge, user, session, now,
        )
    }
    fn approve(
        &self,
        transaction: &str,
        client: &str,
        scopes: Vec<OidcScope>,
        user: UserId,
        session: SessionId,
        now: UnixTimestamp,
    ) -> Result<ApprovedAuthorization, OidcProtocolError> {
        self.approve(transaction, client, scopes, user, session, now)
    }
    fn exchange(
        &self,
        client: &str,
        secret: Option<&str>,
        code: &str,
        redirect: &str,
        verifier: &str,
        now: UnixTimestamp,
    ) -> Result<OidcTokenResponse, OidcProtocolError> {
        self.exchange_code(client, secret, code, redirect, verifier, now)
    }
    fn userinfo(&self, token: &str, now: UnixTimestamp) -> Result<Value, OidcProtocolError> {
        self.userinfo(token, now)
    }
    fn discovery(&self) -> Value {
        self.discovery()
    }
    fn jwks(&self) -> Value {
        self.jwks()
    }
}

pub(super) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/oauth2/jwks", get(jwks))
        .route("/oauth2/authorize", get(authorize))
        .route("/oauth2/authorize/approve", post(approve))
        .route("/oauth2/token", post(token))
        .route("/oauth2/userinfo", get(userinfo))
}

async fn discovery(State(state): State<Arc<GatewayState>>) -> Response {
    let Some(oidc) = &state.oidc else {
        return unavailable();
    };
    json_response(StatusCode::OK, oidc.discovery())
}

async fn jwks(State(state): State<Arc<GatewayState>>) -> Response {
    let Some(oidc) = &state.oidc else {
        return unavailable();
    };
    json_response(StatusCode::OK, oidc.jwks())
}

async fn authorize(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let Some(oidc) = state.oidc.as_ref() else {
        return unavailable();
    };
    let Some(fields) = uri.query().and_then(parse_fields) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if field(&fields, "response_type") != Some("code")
        || field(&fields, "code_challenge_method") != Some("S256")
    {
        return oauth_error(StatusCode::BAD_REQUEST, "unsupported_response_type");
    }
    let Some((client, redirect, scopes, request_state, nonce, challenge)) = (|| {
        Some((
            field(&fields, "client_id")?,
            field(&fields, "redirect_uri")?,
            parse_scopes(field(&fields, "scope")?)?,
            field(&fields, "state")?,
            field(&fields, "nonce")?,
            field(&fields, "code_challenge")?,
        ))
    })() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Ok(cookie) = credential(&headers) else {
        return oauth_error(StatusCode::UNAUTHORIZED, "login_required");
    };
    let Ok(identity) = authenticate(&state, &cookie).await else {
        return oauth_error(StatusCode::UNAUTHORIZED, "login_required");
    };
    let Ok(now) = UnixTimestamp::now() else {
        return unavailable();
    };
    let oidc = Arc::clone(oidc);
    let client = client.to_owned();
    let redirect = redirect.to_owned();
    let request_state = request_state.to_owned();
    let nonce = nonce.to_owned();
    let challenge = challenge.to_owned();
    let result = tokio::task::spawn_blocking(move || {
        let display_name = oidc.client_display_name(&client)?;
        oidc.begin(
            &client,
            &redirect,
            scopes.clone(),
            &request_state,
            &nonce,
            &challenge,
            identity.user_id,
            identity.session_id,
            now,
        )
        .map(|transaction| (transaction, client, scopes, display_name))
    })
    .await;
    let (transaction, client, scopes, display_name) = match result {
        Ok(Ok(values)) => values,
        Ok(Err(OidcProtocolError::Unavailable)) | Err(_) => return unavailable(),
        Ok(Err(OidcProtocolError::UnsupportedScope)) => {
            return oauth_error(StatusCode::BAD_REQUEST, "invalid_scope");
        }
        _ => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    // Transaction/client IDs are generated, scopes are known names, and the
    // manifest-derived display name is escaped. Redirect/state are not rendered.
    let scopes = scopes
        .iter()
        .map(|scope| scope.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let display_name = escape_html(&display_name);
    let german = headers
        .get("accept-language")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with("de"));
    let (language, title, description, allow, deny) = if german {
        (
            "de",
            "App autorisieren",
            "Dieser App folgenden Zugriff erlauben",
            "Erlauben",
            "Zum Ablehnen diese Seite schließen.",
        )
    } else {
        (
            "en",
            "Authorize app",
            "Allow this app to use",
            "Allow",
            "Close this page to deny access.",
        )
    };
    let body = format!(
        "<!doctype html><html lang=\"{language}\"><meta charset=\"utf-8\"><title>{title} · rumahl OS</title><h1>{title}</h1><p>{description} – {display_name}: {scopes}</p><form method=\"post\" action=\"/oauth2/authorize/approve\"><input type=\"hidden\" name=\"transaction\" value=\"{transaction}\"><input type=\"hidden\" name=\"client_id\" value=\"{client}\"><input type=\"hidden\" name=\"scope\" value=\"{scopes}\"><button type=\"submit\">{allow}</button></form><p>{deny}</p></html>"
    );
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
        ),
    );
    secure_headers(&mut response);
    response
}

async fn approve(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(oidc) = state.oidc.as_ref() else {
        return unavailable();
    };
    if !valid_origin(&headers, &state.config.public_origin) {
        return oauth_error(StatusCode::FORBIDDEN, "invalid_request");
    }
    let Some(fields) = parse_form(&headers, &body) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Some((transaction, client, scopes)) = (|| {
        Some((
            field(&fields, "transaction")?,
            field(&fields, "client_id")?,
            parse_scopes(field(&fields, "scope")?)?,
        ))
    })() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Ok(cookie) = credential(&headers) else {
        return oauth_error(StatusCode::UNAUTHORIZED, "login_required");
    };
    let Ok(identity) = authenticate(&state, &cookie).await else {
        return oauth_error(StatusCode::UNAUTHORIZED, "login_required");
    };
    let Ok(now) = UnixTimestamp::now() else {
        return unavailable();
    };
    let oidc = Arc::clone(oidc);
    let transaction = transaction.to_owned();
    let client = client.to_owned();
    let result = tokio::task::spawn_blocking(move || {
        oidc.approve(
            &transaction,
            &client,
            scopes,
            identity.user_id,
            identity.session_id,
            now,
        )
    })
    .await;
    let approved = match result {
        Ok(Ok(approved)) => approved,
        Ok(Err(OidcProtocolError::Unavailable)) | Err(_) => return unavailable(),
        _ => return oauth_error(StatusCode::FORBIDDEN, "access_denied"),
    };
    let Ok(mut redirect) = url::Url::parse(approved.redirect_uri().as_str()) else {
        return unavailable();
    };
    redirect
        .query_pairs_mut()
        .append_pair("code", approved.code())
        .append_pair("state", approved.state());
    let Ok(location) = HeaderValue::from_str(redirect.as_str()) else {
        return unavailable();
    };
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response.headers_mut().insert(LOCATION, location);
    secure_headers(&mut response);
    response
}

async fn token(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(oidc) = state.oidc.as_ref() else {
        return unavailable();
    };
    let Some(fields) = parse_form(&headers, &body) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if field(&fields, "grant_type") != Some("authorization_code") {
        return oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type");
    }
    let Some((code, redirect, verifier)) = (|| {
        Some((
            field(&fields, "code")?,
            field(&fields, "redirect_uri")?,
            field(&fields, "code_verifier")?,
        ))
    })() else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let basic = match basic_credentials(&headers) {
        Ok(basic) => basic,
        Err(()) => return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"),
    };
    let client = match &basic {
        Some((id, _)) => {
            if field(&fields, "client_id").is_some_and(|body_id| body_id != id) {
                return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client");
            }
            id.as_str()
        }
        None => match field(&fields, "client_id") {
            Some(id) => id,
            None => return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"),
        },
    };
    let Ok(now) = UnixTimestamp::now() else {
        return unavailable();
    };
    let oidc = Arc::clone(oidc);
    let client = client.to_owned();
    let code = code.to_owned();
    let redirect = redirect.to_owned();
    let verifier = verifier.to_owned();
    let result = tokio::task::spawn_blocking(move || {
        oidc.exchange(
            &client,
            basic.as_ref().map(|(_, secret)| secret.as_str()),
            &code,
            &redirect,
            &verifier,
            now,
        )
    })
    .await;
    match result {
        Ok(Ok(tokens)) => json_response(StatusCode::OK, tokens.as_json()),
        Ok(Err(error)) => protocol_error(error),
        Err(_) => unavailable(),
    }
}

async fn userinfo(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    let Some(oidc) = state.oidc.as_ref() else {
        return unavailable();
    };
    let Some(token) = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_token");
    };
    if headers.get_all(AUTHORIZATION).iter().count() != 1 || token.len() != 43 {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_token");
    }
    let Ok(now) = UnixTimestamp::now() else {
        return unavailable();
    };
    let oidc = Arc::clone(oidc);
    let token = token.to_owned();
    match tokio::task::spawn_blocking(move || oidc.userinfo(&token, now)).await {
        Ok(Ok(claims)) => json_response(StatusCode::OK, claims),
        Ok(Err(OidcProtocolError::Unavailable)) | Err(_) => unavailable(),
        _ => oauth_error(StatusCode::UNAUTHORIZED, "invalid_token"),
    }
}

fn parse_form(headers: &HeaderMap, bytes: &[u8]) -> Option<HashMap<String, String>> {
    if headers.get(CONTENT_TYPE)?.to_str().ok()? != "application/x-www-form-urlencoded"
        || bytes.len() > MAX_FORM_BYTES
    {
        return None;
    }
    parse_fields(std::str::from_utf8(bytes).ok()?)
}

fn parse_fields(encoded: &str) -> Option<HashMap<String, String>> {
    if encoded.len() > MAX_FORM_BYTES {
        return None;
    }
    let mut fields = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(encoded.as_bytes()) {
        if fields
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return None;
        }
    }
    Some(fields)
}

fn field<'a>(fields: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    fields.get(name).map(String::as_str)
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn parse_scopes(raw: &str) -> Option<Vec<OidcScope>> {
    let scopes = raw
        .split_ascii_whitespace()
        .map(OidcScope::parse)
        .collect::<Option<Vec<_>>>()?;
    if scopes.is_empty() || scopes.len() > 4 {
        return None;
    }
    Some(scopes)
}

fn basic_credentials(headers: &HeaderMap) -> Result<Option<(String, Zeroizing<String>)>, ()> {
    if headers.get_all(AUTHORIZATION).iter().count() > 1 {
        return Err(());
    }
    let Some(raw) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let encoded = raw
        .to_str()
        .map_err(|_| ())?
        .strip_prefix("Basic ")
        .ok_or(())?;
    if encoded.len() > 512 {
        return Err(());
    }
    let decoded = Zeroizing::new(STANDARD.decode(encoded).map_err(|_| ())?);
    let decoded = std::str::from_utf8(&decoded).map_err(|_| ())?;
    let (id, secret) = decoded.split_once(':').ok_or(())?;
    if id.is_empty() || secret.is_empty() {
        return Err(());
    }
    Ok(Some((id.to_owned(), Zeroizing::new(secret.to_owned()))))
}

fn protocol_error(error: OidcProtocolError) -> Response {
    match error {
        OidcProtocolError::InvalidClient => oauth_error(StatusCode::UNAUTHORIZED, "invalid_client"),
        OidcProtocolError::InvalidGrant | OidcProtocolError::AccessDenied => {
            oauth_error(StatusCode::BAD_REQUEST, "invalid_grant")
        }
        OidcProtocolError::UnsupportedScope => {
            oauth_error(StatusCode::BAD_REQUEST, "invalid_scope")
        }
        OidcProtocolError::InvalidRequest => {
            oauth_error(StatusCode::BAD_REQUEST, "invalid_request")
        }
        OidcProtocolError::Unavailable => unavailable(),
    }
}

fn oauth_error(status: StatusCode, code: &str) -> Response {
    let mut response = json_response(status, json!({"error": code}));
    if status == StatusCode::UNAUTHORIZED && code == "invalid_client" {
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"rumahl OIDC\""),
        );
    }
    response
}

fn unavailable() -> Response {
    oauth_error(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable")
}

fn json_response(status: StatusCode, body: Value) -> Response {
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    secure_headers(&mut response);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use rumahl_core::{
        AccountState, AccountStateRepository, AppId, InstallationId, OidcClientType,
    };
    use rumahl_oidc_provider::{
        OidcClientId, OidcClientRecord, OidcClientSecret, OidcRedirectUri, OidcSigningKey,
    };
    use rumahl_persistence_sqlite::{
        SqliteAccountStateRepository, SqliteOidcAccessTokenStore, SqliteOidcAuthorizationStore,
        SqliteOidcClientRepository,
    };
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;

    use crate::{
        GatewayConfig, InMemoryShellEvents, ShellBackend, ShellBackendError, ShellIdentity,
        WidgetFrameResolver, router,
    };

    const COOKIE_VALUE: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~";

    struct TestLogin(ShellIdentity);

    impl ShellBackend for TestLogin {
        fn authenticate(&self, _: &str) -> Result<ShellIdentity, ShellBackendError> {
            Ok(self.0)
        }
        fn snapshot(
            &self,
            _: &str,
        ) -> Result<rumahl_ui_contracts::ShellSnapshot, ShellBackendError> {
            Err(ShellBackendError::Unavailable)
        }
    }

    struct NoWidgets;
    impl WidgetFrameResolver for NoWidgets {
        fn resolve(&self, _: UserId, _: &str, _: &str) -> Option<String> {
            None
        }
    }

    #[test]
    fn duplicate_parameters_and_unsupported_scopes_fail_closed() {
        assert!(parse_fields("state=one&state=two").is_none());
        assert!(parse_scopes("openid admin").is_none());
        assert_eq!(
            parse_scopes("openid profile"),
            Some(vec![OidcScope::OpenId, OidcScope::Profile])
        );
        assert_eq!(escape_html("A <B> & 'C'"), "A &lt;B&gt; &amp; &#39;C&#39;");
    }

    #[test]
    fn basic_client_auth_rejects_bad_header_and_duplicate_header() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer x"));
        assert!(basic_credentials(&headers).is_err());
        headers.append(AUTHORIZATION, HeaderValue::from_static("Basic eDp5"));
        assert!(basic_credentials(&headers).is_err());
    }

    #[tokio::test]
    async fn http_code_flow_uses_live_cookie_sqlite_pkce_and_one_use_token() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oidc.sqlite3");
        let now = UnixTimestamp::now().unwrap();
        let auth_time = UnixTimestamp::from_seconds(now.as_seconds() - 10);
        let expiry = UnixTimestamp::from_seconds(now.as_seconds() + 3600);
        let accounts = SqliteAccountStateRepository::open(&path).unwrap();
        let mut account_state = AccountState::new();
        let user_id = account_state.create_account("alice", "Alice").unwrap();
        let session_id = account_state
            .start_session(&user_id, auth_time, expiry)
            .unwrap();
        accounts.store(&account_state).unwrap();
        let client_secret = OidcClientSecret::generate().unwrap();
        let client = OidcClientRecord::restore(
            OidcClientId::generate().unwrap(),
            InstallationId::new(),
            AppId::parse("com.rumahl.cloud").unwrap(),
            "Cloud",
            OidcClientType::Confidential,
            OidcRedirectUri::parse("https://cloud.rumahl.dev/apps/user_oidc/code").unwrap(),
            vec![OidcScope::OpenId, OidcScope::Profile],
            Some(client_secret.digest()),
            auth_time,
            None,
        )
        .unwrap();
        SqliteOidcClientRepository::open(&path)
            .unwrap()
            .insert(&client)
            .unwrap();
        let protocol = OidcProtocol::new(
            "https://rumahl.dev",
            SqliteOidcClientRepository::open(&path).unwrap(),
            SqliteOidcAuthorizationStore::open(&path).unwrap(),
            SqliteOidcAccessTokenStore::open(&path).unwrap(),
            SqliteAccountStateRepository::open(&path).unwrap(),
            OidcSigningKey::new("key-1", [7; 32]).unwrap(),
        )
        .unwrap();
        let app = router(GatewayState {
            config: GatewayConfig::new("https://rumahl.dev", "/private/run/rumahl-ssr.sock")
                .unwrap(),
            backend: Arc::new(TestLogin(ShellIdentity {
                user_id,
                session_id,
            })),
            events: Arc::new(InMemoryShellEvents::new(4)),
            widgets: Arc::new(NoWidgets),
            streams: None,
            oidc: Some(Arc::new(protocol)),
        });
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(VERIFIER.as_bytes()));
        let authorize_uri = format!(
            "/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile&state=state-12345678&nonce=nonce-12345678&code_challenge={challenge}&code_challenge_method=S256",
            client.client_id().as_str(),
            url::form_urlencoded::byte_serialize(client.redirect_uri().as_str().as_bytes())
                .collect::<String>(),
        );
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&authorize_uri)
                    .header("cookie", format!("__Host-rumahl_session={COOKIE_VALUE}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(html.contains("Cloud"));
        let transaction = html
            .split("name=\"transaction\" value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let approval = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("transaction", transaction)
            .append_pair("client_id", client.client_id().as_str())
            .append_pair("scope", "openid profile")
            .finish();
        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/oauth2/authorize/approve")
                    .header("cookie", format!("__Host-rumahl_session={COOKIE_VALUE}"))
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header("origin", "https://evil.rumahl.dev")
                    .body(Body::from(approval.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let approved = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/oauth2/authorize/approve")
                    .header("cookie", format!("__Host-rumahl_session={COOKIE_VALUE}"))
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header("origin", "https://rumahl.dev")
                    .body(Body::from(approval))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approved.status(), StatusCode::SEE_OTHER);
        let location = approved.headers()[LOCATION].to_str().unwrap();
        let redirect = url::Url::parse(location).unwrap();
        assert_eq!(
            redirect.origin().ascii_serialization(),
            "https://cloud.rumahl.dev"
        );
        let code = redirect
            .query_pairs()
            .find(|(key, _)| key == "code")
            .unwrap()
            .1
            .into_owned();
        let token_form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("client_id", client.client_id().as_str())
            .append_pair("code", &code)
            .append_pair("redirect_uri", client.redirect_uri().as_str())
            .append_pair("code_verifier", VERIFIER)
            .finish();
        let basic = format!(
            "Basic {}",
            STANDARD.encode(format!(
                "{}:{}",
                client.client_id().as_str(),
                client_secret.encode().as_str()
            ))
        );
        let token_request = || {
            Request::builder()
                .method("POST")
                .uri("/oauth2/token")
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(AUTHORIZATION, &basic)
                .body(Body::from(token_form.clone()))
                .unwrap()
        };
        let bad_client = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/oauth2/token")
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header(
                        AUTHORIZATION,
                        format!(
                            "Basic {}",
                            STANDARD.encode(format!("{}:wrong", client.client_id().as_str()))
                        ),
                    )
                    .body(Body::from(token_form.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad_client.status(), StatusCode::UNAUTHORIZED);
        let tokens = app.clone().oneshot(token_request()).await.unwrap();
        assert_eq!(tokens.status(), StatusCode::OK);
        let tokens: Value =
            serde_json::from_slice(&tokens.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let access = tokens["access_token"].as_str().unwrap();
        let info = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/oauth2/userinfo")
                    .header(AUTHORIZATION, format!("Bearer {access}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(info.status(), StatusCode::OK);
        let info: Value =
            serde_json::from_slice(&info.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(info["name"], "Alice");
        assert_eq!(
            app.oneshot(token_request()).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }
}
