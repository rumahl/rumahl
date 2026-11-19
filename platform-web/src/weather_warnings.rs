//! OS-owned severe-weather warnings.
//!
//! The route surface is deliberately small: every authenticated client may read
//! active warnings, while only an installed app that declared
//! `com.rumahl.os.warnings` (and, for emergency level, also
//! `com.rumahl.os.emergency`) may register or publish. Apps can never choose
//! another source id: the shell bridge supplies their own catalog id and the
//! server authorizes it again against the current installation and capability
//! list.
use crate::{
    GatewayState, ShellBackendError, ShellIdentity,
    preferences::{error, user},
    server::{authenticate, credential, secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rumahl_core::{
    AppId, OS_WARNING_SOURCE_ID, WEATHER_WARNING_CAPABILITY, WarningLevel, WarningSource,
    WeatherWarning, WeatherWarningDraft, WeatherWarningError,
};
use rumahl_ui_contracts::ShellEvent;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

const MAX_PUBLISH_BATCH: usize = 8;
const MAX_TEST_LIFETIME_MS: i64 = 2 * 60 * 1_000;

pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route("/api/v1/shell/warnings", get(active))
        .route("/api/v1/shell/warnings/register", post(register))
        .route("/api/v1/shell/warnings/publish", post(publish))
        .route("/api/v1/shell/warnings/clear", post(clear))
        .route("/api/v1/shell/warnings/test", post(test_warning))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegisterBody {
    app_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishBody {
    app_id: String,
    warnings: Vec<WireWarning>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClearBody {
    app_id: String,
    #[serde(default)]
    ids: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TestBody {
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    headline: Option<String>,
    #[serde(default)]
    description: Option<String>,
    /// Marks the test as an extreme scenario so the shell can exercise the
    /// fullscreen/idle path without publishing a real life-threatening warning.
    #[serde(default)]
    extreme: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireWarning {
    id: String,
    level: String,
    headline: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    region: String,
    #[serde(default)]
    event: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    sender: String,
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    area: String,
    #[serde(default)]
    effective: Option<i64>,
    expires: i64,
    #[serde(default)]
    test: bool,
}

impl WireWarning {
    fn draft(self, source_id: &str) -> Result<WeatherWarningDraft, WeatherWarningError> {
        let level = WarningLevel::parse(&self.level).ok_or(WeatherWarningError::Invalid)?;
        Ok(WeatherWarningDraft {
            source_id: source_id.to_owned(),
            id: self.id,
            level,
            headline: self.headline,
            description: self.description,
            region: self.region,
            event: self.event,
            severity: self.severity,
            sender: self.sender,
            instruction: self.instruction,
            area: self.area,
            effective_at_ms: self.effective,
            expires_at_ms: self.expires,
            test: self.test,
        })
    }
}

fn now_ms() -> i64 {
    rumahl_core::UnixTimestamp::now().map_or(0, |now| {
        i64::try_from(now.as_seconds())
            .unwrap_or(i64::MAX / 1_000)
            .saturating_mul(1_000)
    })
}

fn auth_error(failure: ShellBackendError) -> Response {
    error(match failure {
        ShellBackendError::Unauthorized => StatusCode::UNAUTHORIZED,
        ShellBackendError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}

async fn identity(
    state: &Arc<GatewayState>,
    headers: &HeaderMap,
) -> Result<ShellIdentity, Response> {
    let token = credential(headers).map_err(|_| error(StatusCode::UNAUTHORIZED))?;
    authenticate(state, &token).await.map_err(auth_error)
}

fn require_json(headers: &HeaderMap, state: &GatewayState) -> Option<Response> {
    if !valid_origin(headers, &state.config.public_origin) {
        return Some(error(StatusCode::FORBIDDEN));
    }
    if headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        != Some("application/json")
    {
        return Some(error(StatusCode::UNSUPPORTED_MEDIA_TYPE));
    }
    None
}

fn warning_error(value: WeatherWarningError) -> Response {
    error(match value {
        WeatherWarningError::Invalid | WeatherWarningError::UnknownSource => {
            StatusCode::BAD_REQUEST
        }
        WeatherWarningError::Limit => StatusCode::TOO_MANY_REQUESTS,
        WeatherWarningError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}

fn json(value: serde_json::Value) -> Response {
    let mut response = axum::Json(value).into_response();
    secure_headers(&mut response);
    response
}

fn source_json(source: &WarningSource) -> serde_json::Value {
    serde_json::json!({
        "id": source.app_id,
        "name": source.display_name,
        "registeredAt": source.registered_at_ms
    })
}

fn warning_json(warning: &WeatherWarning) -> serde_json::Value {
    serde_json::json!({
        "id": warning.id,
        "source": source_json(&warning.source),
        "level": warning.level.as_str(),
        "headline": warning.headline,
        "description": warning.description,
        "region": warning.region,
        "event": warning.event,
        "severity": warning.severity,
        "sender": warning.sender,
        "instruction": warning.instruction,
        "area": warning.area,
        "effective": warning.effective_at_ms,
        "expires": warning.expires_at_ms,
        "test": warning.test,
        "publishedAt": warning.published_at_ms,
        "updatedAt": warning.updated_at_ms
    })
}

fn warning_revision(warnings: &[WeatherWarning]) -> String {
    let mut digest = Sha256::new();
    for warning in warnings {
        digest.update(warning.id.as_bytes());
        digest.update([0]);
        digest.update(warning.source.app_id.as_bytes());
        digest.update([0]);
        digest.update(warning.level.as_str().as_bytes());
        digest.update(warning.updated_at_ms.to_be_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn warnings_response(warnings: Vec<WeatherWarning>) -> Response {
    let revision = warning_revision(&warnings);
    let values: Vec<_> = warnings.iter().map(warning_json).collect();
    json(serde_json::json!({
        "warningsVersion": 1,
        "revision": revision,
        "observedAt": now_ms(),
        "warnings": values
    }))
}

async fn active(State(state): State<Arc<GatewayState>>, headers: HeaderMap) -> Response {
    if let Err(response) = user(&state, &headers).await {
        return response;
    }
    let Some(repository) = state.weather_warnings.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let now = now_ms();
    match tokio::task::spawn_blocking(move || repository.active(now)).await {
        Ok(Ok(warnings)) => warnings_response(warnings),
        Ok(Err(value)) => warning_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn authorized_app(
    state: &Arc<GatewayState>,
    identity: ShellIdentity,
    app_id: &str,
) -> Result<crate::CatalogApp, Response> {
    AppId::parse(app_id).map_err(|_| error(StatusCode::BAD_REQUEST))?;
    let Some(apps) = state.apps.clone() else {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE));
    };
    let id = app_id.to_owned();
    let catalog = tokio::task::spawn_blocking(move || apps.catalog(identity))
        .await
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE))?
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE))?;
    let app = catalog
        .into_iter()
        .find(|app| app.id == id)
        .ok_or_else(|| error(StatusCode::FORBIDDEN))?;
    if !app
        .capabilities
        .iter()
        .any(|capability| capability == WEATHER_WARNING_CAPABILITY)
    {
        return Err(error(StatusCode::FORBIDDEN));
    }
    Ok(app)
}

async fn register(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(response) = require_json(&headers, &state) {
        return response;
    }
    let identity = match identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let body: RegisterBody = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let app = match authorized_app(&state, identity, &body.app_id).await {
        Ok(app) => app,
        Err(response) => return response,
    };
    let Some(repository) = state.weather_warnings.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let app_id = app.id.clone();
    let title = app.title.clone();
    let now = now_ms();
    match tokio::task::spawn_blocking(move || repository.register_source(&app_id, &title, now))
        .await
    {
        Ok(Ok(source)) => json(serde_json::json!({
            "registrationVersion": 1,
            "source": source_json(&source)
        })),
        Ok(Err(value)) => warning_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn publish(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(response) = require_json(&headers, &state) {
        return response;
    }
    let identity = match identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let body: PublishBody = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    if body.warnings.is_empty() || body.warnings.len() > MAX_PUBLISH_BATCH {
        return error(StatusCode::BAD_REQUEST);
    }
    let app = match authorized_app(&state, identity, &body.app_id).await {
        Ok(app) => app,
        Err(response) => return response,
    };
    let Some(repository) = state.weather_warnings.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let now = now_ms();
    let app_id = app.id.clone();
    let title = app.title.clone();
    let mut drafts = Vec::with_capacity(body.warnings.len());
    let mut seen = std::collections::HashSet::new();
    for warning in body.warnings {
        if !seen.insert(warning.id.clone()) {
            return error(StatusCode::BAD_REQUEST);
        }
        let draft = match warning.draft(&app_id) {
            Ok(draft) => draft,
            Err(value) => return warning_error(value),
        };
        match draft.validate(now) {
            Ok(draft) => drafts.push(draft),
            Err(value) => return warning_error(value),
        }
    }
    let result = tokio::task::spawn_blocking(move || {
        repository.register_source(&app_id, &title, now)?;
        let mut published = Vec::with_capacity(drafts.len());
        for draft in drafts {
            published.push(repository.publish(draft, now)?);
        }
        Ok::<_, WeatherWarningError>(published)
    })
    .await;
    match result {
        Ok(Ok(published)) => {
            state.events.publish_all(ShellEvent::warnings_changed());
            json(serde_json::json!({
                "publishVersion": 1,
                "accepted": published.len(),
                "warnings": published.iter().map(warning_json).collect::<Vec<_>>()
            }))
        }
        Ok(Err(value)) => warning_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn clear(
    State(state): State<Arc<GatewayState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(response) = require_json(&headers, &state) {
        return response;
    }
    let identity = match identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let body: ClearBody = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    if body.ids.as_ref().is_some_and(|ids| ids.len() > 32) {
        return error(StatusCode::BAD_REQUEST);
    }
    let app = match authorized_app(&state, identity, &body.app_id).await {
        Ok(app) => app,
        Err(response) => return response,
    };
    let Some(repository) = state.weather_warnings.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let app_id = app.id.clone();
    let ids = body.ids;
    let now = now_ms();
    let result =
        tokio::task::spawn_blocking(move || repository.clear(&app_id, ids.as_deref(), now)).await;
    match result {
        Ok(Ok(cleared)) => {
            state.events.publish_all(ShellEvent::warnings_changed());
            json(serde_json::json!({
                "clearVersion": 1,
                "cleared": cleared
            }))
        }
        Ok(Err(value)) => warning_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn test_warning(
    State(state): State<Arc<GatewayState>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(response) = require_json(&headers, &state) {
        return response;
    }
    if query.len() != 1 {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(device) = query
        .get("device")
        .and_then(|value| rumahl_core::BrowserProfileId::parse(value))
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    let identity = match identity(&state, &headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    let Some(mode_repository) = state.os_mode.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let user_id = identity.user_id;
    let mode =
        match tokio::task::spawn_blocking(move || mode_repository.load(user_id, device)).await {
            Ok(Ok(settings)) => settings.effective_mode(),
            Ok(Err(_)) => return error(StatusCode::SERVICE_UNAVAILABLE),
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE),
        };
    if !mode.policy().can_publish_test_warnings() {
        return error(StatusCode::FORBIDDEN);
    }
    let body: TestBody = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => return error(StatusCode::BAD_REQUEST),
    };
    let level = match body.level.as_deref() {
        None => WarningLevel::Warning,
        Some(value) => match WarningLevel::parse(value) {
            Some(level) => level,
            None => return error(StatusCode::BAD_REQUEST),
        },
    };
    let now = now_ms();
    let extreme = body.extreme.unwrap_or(false);
    let draft = WeatherWarningDraft {
        source_id: OS_WARNING_SOURCE_ID.to_owned(),
        id: "os-test".to_owned(),
        level,
        headline: body
            .headline
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                if extreme {
                    "Test tornado warning".to_owned()
                } else {
                    "Test warning".to_owned()
                }
            }),
        description: body.description.unwrap_or_else(|| {
            "This is a test warning from rumahl OS. It expires automatically.".to_owned()
        }),
        region: "Test region".to_owned(),
        event: if extreme {
            "TORNADO".to_owned()
        } else {
            "TEST".to_owned()
        },
        severity: if extreme {
            "Extreme".to_owned()
        } else {
            "Test".to_owned()
        },
        sender: "rumahl OS".to_owned(),
        instruction: "No action is required.".to_owned(),
        area: String::new(),
        effective_at_ms: Some(now),
        expires_at_ms: now + MAX_TEST_LIFETIME_MS,
        test: true,
    };
    let Some(repository) = state.weather_warnings.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let result = tokio::task::spawn_blocking(move || {
        repository.register_source(OS_WARNING_SOURCE_ID, "rumahl OS", now)?;
        repository.publish(draft, now)
    })
    .await;
    match result {
        Ok(Ok(warning)) => {
            state.events.publish_all(ShellEvent::warnings_changed());
            json(serde_json::json!({
                "publishVersion": 1,
                "accepted": 1,
                "warnings": [warning_json(&warning)]
            }))
        }
        Ok(Err(value)) => warning_error(value),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppAccess, AppAsset, AppData, AppProvider, AppSettingInfo, CatalogApp, GatewayConfig,
        InMemoryShellEvents, ShellBackend, ShellBackendError, ShellIdentity, WidgetFrameResolver,
        router,
    };
    use axum::body::Body;
    use axum::http::Request;
    use rumahl_core::{
        BrowserProfileId, InstallationId, OsMode, OsModeRepository, OsModeSettings,
        OsModeStoreError, PreferenceScope, SessionId, UserId, WeatherWarningRepository,
    };
    use rumahl_persistence_sqlite::SqliteWeatherWarnings;
    use rumahl_ui_contracts::ShellSnapshot;
    use std::path::Path;
    use tower::ServiceExt;

    const TOKEN: &str = "0123456789012345678901234567890123456789012";
    const DEVICE: &str = "00000000-0000-4000-8000-000000000001";

    struct TestBackend(ShellIdentity);

    impl ShellBackend for TestBackend {
        fn authenticate(&self, credential: &str) -> Result<ShellIdentity, ShellBackendError> {
            if credential == TOKEN {
                Ok(self.0)
            } else {
                Err(ShellBackendError::Unauthorized)
            }
        }

        fn snapshot(
            &self,
            _credential: &str,
            _device: Option<BrowserProfileId>,
        ) -> Result<ShellSnapshot, ShellBackendError> {
            Err(ShellBackendError::Unavailable)
        }
    }

    struct TestWidgets;

    impl WidgetFrameResolver for TestWidgets {
        fn resolve(&self, _: UserId, _: &str, _: &str) -> Option<String> {
            None
        }
    }

    struct TestApps {
        capabilities: Vec<String>,
    }

    impl AppProvider for TestApps {
        fn catalog(
            &self,
            _identity: ShellIdentity,
        ) -> Result<Vec<CatalogApp>, crate::AppAccessError> {
            Ok(vec![CatalogApp {
                id: "com.example.weather".to_owned(),
                installation_id: InstallationId::new(),
                title: "Weather".to_owned(),
                version: "1.0.0".to_owned(),
                launchable: false,
                capabilities: self.capabilities.clone(),
                runtime: "web".to_owned(),
                lifecycle: "on-demand".to_owned(),
                connectors: Vec::new(),
            }])
        }

        fn entrypoint(
            &self,
            _identity: ShellIdentity,
            _installation: InstallationId,
        ) -> Result<String, crate::AppAccessError> {
            Err(crate::AppAccessError::Denied)
        }

        fn asset(
            &self,
            _identity: ShellIdentity,
            _installation: InstallationId,
            _path: &str,
        ) -> Result<AppAsset, crate::AppAccessError> {
            Err(crate::AppAccessError::Denied)
        }

        fn settings(
            &self,
            _identity: ShellIdentity,
            _installation: InstallationId,
        ) -> Result<Vec<AppSettingInfo>, crate::AppAccessError> {
            Ok(Vec::new())
        }

        fn data(
            &self,
            _identity: ShellIdentity,
            _installation: InstallationId,
            _path: &str,
        ) -> Result<AppData, crate::AppAccessError> {
            Err(crate::AppAccessError::Denied)
        }
    }

    struct TestMode(OsMode);

    impl OsModeRepository for TestMode {
        fn load(
            &self,
            _user: UserId,
            _device: BrowserProfileId,
        ) -> Result<OsModeSettings, OsModeStoreError> {
            Ok(OsModeSettings {
                revision: 0,
                user_mode: self.0,
                device_mode: None,
            })
        }

        fn save(
            &self,
            _user: UserId,
            _device: BrowserProfileId,
            _revision: u64,
            _scope: PreferenceScope,
            _mode: Option<OsMode>,
        ) -> Result<OsModeSettings, OsModeStoreError> {
            Err(OsModeStoreError::Unavailable)
        }
    }

    struct Fixture {
        app: axum::Router,
        warnings: Arc<SqliteWeatherWarnings>,
    }

    fn fixture(capabilities: &[&str], mode: OsMode) -> Fixture {
        let config =
            GatewayConfig::new("https://rumahl.test", "/tmp/rumahl-test-ssr.sock").unwrap();
        let apps = Arc::new(
            AppAccess::new(
                Arc::new(TestApps {
                    capabilities: capabilities
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                }),
                &config.public_origin,
                "apps.rumahl.test",
            )
            .unwrap(),
        );
        let warnings = Arc::new(SqliteWeatherWarnings::open_in_memory().unwrap());
        let state = GatewayState {
            config,
            backend: Arc::new(TestBackend(ShellIdentity {
                user_id: UserId::new(),
                session_id: SessionId::new(),
            })),
            events: Arc::new(InMemoryShellEvents::new(8)),
            widgets: Arc::new(TestWidgets),
            streams: None,
            apps: Some(apps),
            files: None,
            workspace: None,
            preferences: None,
            os_mode: Some(Arc::new(TestMode(mode))),
            host_files: None,
            weather_warnings: Some(warnings.clone()),
            oidc: None,
            browser_sessions: None,
            admin: None,
            login_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            audit: None,
            state_dir: Some(std::path::PathBuf::from(Path::new("/tmp"))),
        };
        Fixture {
            app: router(state),
            warnings,
        }
    }

    fn request(
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
        origin: bool,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("cookie", format!("__Host-rumahl_session={TOKEN}"));
        if origin {
            builder = builder
                .header("origin", "https://rumahl.test")
                .header("content-type", "application/json");
        }
        builder
            .body(Body::from(
                body.map_or(String::new(), |value| value.to_string()),
            ))
            .unwrap()
    }

    fn warning() -> serde_json::Value {
        serde_json::json!({
            "id": "storm-1",
            "level": "warning",
            "headline": "Severe storm",
            "description": "Gusts up to 120 km/h.",
            "region": "Berlin",
            "expires": now_ms() + 3_600_000
        })
    }

    #[tokio::test]
    async fn warnings_require_a_session_origin_and_declared_capability() {
        let fixture = fixture(&[], OsMode::Guided);
        let unauthorized = fixture
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/shell/warnings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let no_origin = fixture
            .app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/shell/warnings/publish",
                Some(serde_json::json!({
                    "appId": "com.example.weather",
                    "warnings": [warning()]
                })),
                false,
            ))
            .await
            .unwrap();
        assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);

        let denied = fixture
            .app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/shell/warnings/publish",
                Some(serde_json::json!({
                    "appId": "com.example.weather",
                    "warnings": [warning()]
                })),
                true,
            ))
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert!(fixture.warnings.active(now_ms()).unwrap().is_empty());
    }

    #[tokio::test]
    async fn declared_apps_publish_and_clear_their_own_warnings() {
        let fixture = fixture(&[WEATHER_WARNING_CAPABILITY], OsMode::Guided);
        let published = fixture
            .app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/shell/warnings/publish",
                Some(serde_json::json!({
                    "appId": "com.example.weather",
                    "warnings": [warning()]
                })),
                true,
            ))
            .await
            .unwrap();
        assert_eq!(published.status(), StatusCode::OK);
        let body = axum::body::to_bytes(published.into_body(), 16 * 1024)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["accepted"], 1);

        let active = fixture
            .app
            .clone()
            .oneshot(request("GET", "/api/v1/shell/warnings", None, false))
            .await
            .unwrap();
        assert_eq!(active.status(), StatusCode::OK);
        assert_eq!(active.headers()["cache-control"], "private, no-store");
        let body = axum::body::to_bytes(active.into_body(), 32 * 1024)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["warnings"][0]["id"], "storm-1");
        assert_eq!(value["warnings"][0]["source"]["id"], "com.example.weather");

        let cleared = fixture
            .app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/shell/warnings/clear",
                Some(serde_json::json!({
                    "appId": "com.example.weather",
                    "ids": ["storm-1"]
                })),
                true,
            ))
            .await
            .unwrap();
        assert_eq!(cleared.status(), StatusCode::OK);
        assert!(fixture.warnings.active(now_ms()).unwrap().is_empty());
    }

    #[tokio::test]
    async fn emergency_level_weather_warnings_do_not_use_the_emergency_boundary() {
        // A weather warning is OS data, not an app emergency notification. The
        // warning capability alone is enough; no emergency capability is needed.
        let fixture = fixture(&[WEATHER_WARNING_CAPABILITY], OsMode::Guided);
        let response = fixture
            .app
            .oneshot(request(
                "POST",
                "/api/v1/shell/warnings/publish",
                Some(serde_json::json!({
                    "appId": "com.example.weather",
                    "warnings": [{
                        "id": "evacuate",
                        "level": "emergency",
                        "headline": "Evacuate now",
                        "expires": now_ms() + 3_600_000
                    }]
                })),
                true,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let active = fixture.warnings.active(now_ms()).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].level, WarningLevel::Emergency);
    }

    #[tokio::test]
    async fn extreme_test_warnings_expose_the_tornado_badge_for_fullscreen_testing() {
        let fixture = fixture(&[], OsMode::Developer);
        let response = fixture
            .app
            .oneshot(request(
                "POST",
                &format!("/api/v1/shell/warnings/test?device={DEVICE}"),
                Some(serde_json::json!({
                    "level": "emergency",
                    "extreme": true
                })),
                true,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let active = fixture.warnings.active(now_ms()).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].event, "TORNADO");
        assert_eq!(active[0].severity, "Extreme");
        assert!(active[0].test);
    }

    #[tokio::test]
    async fn os_test_warnings_are_limited_to_developer_mode() {
        for (mode, expected) in [
            (OsMode::Guided, StatusCode::FORBIDDEN),
            (OsMode::Advanced, StatusCode::FORBIDDEN),
            (OsMode::Developer, StatusCode::OK),
        ] {
            let fixture = fixture(&[], mode);
            let response = fixture
                .app
                .oneshot(request(
                    "POST",
                    &format!("/api/v1/shell/warnings/test?device={DEVICE}"),
                    Some(serde_json::json!({
                        "level": "critical"
                    })),
                    true,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            if expected == StatusCode::OK {
                let active = fixture.warnings.active(now_ms()).unwrap();
                assert_eq!(active.len(), 1);
                assert_eq!(active[0].source.app_id, OS_WARNING_SOURCE_ID);
                assert!(active[0].test);
                assert_eq!(active[0].level, WarningLevel::Critical);
            }
        }
    }
}
