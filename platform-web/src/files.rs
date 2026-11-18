//! Personal document API. File IDs never identify host filesystem paths.
use crate::{
    GatewayState,
    preferences::{error, user},
    server::{secure_headers, valid_origin},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use rumahl_core::FileError;
use std::{collections::HashMap, sync::Arc};
pub(crate) fn routes() -> Router<Arc<GatewayState>> {
    Router::new()
        .route(
            "/api/v1/files",
            get(list).post(create).put(relocate).delete(remove),
        )
        .route("/api/v1/files/content", get(download).put(write))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
}
fn fail(e: FileError) -> Response {
    error(match e {
        FileError::Missing => StatusCode::NOT_FOUND,
        FileError::Invalid => StatusCode::BAD_REQUEST,
        FileError::Conflict => StatusCode::CONFLICT,
        FileError::Limit => StatusCode::PAYLOAD_TOO_LARGE,
        FileError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    })
}
fn key(q: &HashMap<String, String>, name: &str) -> Option<String> {
    q.get(name)
        .filter(|v| {
            v.as_str() == "root" || (v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit()))
        })
        .cloned()
}
fn completed(r: Result<Result<(), FileError>, tokio::task::JoinError>) -> Response {
    match r {
        Ok(Ok(())) => error(StatusCode::NO_CONTENT),
        Ok(Err(e)) => fail(e),
        _ => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
async fn list(
    State(s): State<Arc<GatewayState>>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
) -> Response {
    let u = match user(&s, &h).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(p) = key(&q, "parent") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(repo) = s.files.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    let limit = q
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200)
        .clamp(1, 1000);
    let offset = q
        .get("offset")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let search = q
        .get("q")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.len() <= 255);
    match tokio::task::spawn_blocking(move || {
        repo.list_page(u, &p, offset, limit, search.as_deref())
    })
    .await
    {
        Ok(Ok((items, total))) => {
            let mut r = axum::Json(
                items
                    .into_iter()
                    .map(|f| {
                        serde_json::json!({
                            "id": f.id,
                            "parent": f.parent,
                            "name": f.name,
                            "directory": f.directory,
                            "size": f.size,
                            "children": f.children
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .into_response();
            secure_headers(&mut r);
            if let Ok(value) = HeaderValue::from_str(&total.to_string()) {
                r.headers_mut().insert("x-total-count", value);
            }
            r
        }
        Ok(Err(e)) => fail(e),
        _ => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
async fn create(
    State(s): State<Arc<GatewayState>>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
    b: Bytes,
) -> Response {
    if !valid_origin(&h, &s.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let u = match user(&s, &h).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(p) = key(&q, "parent") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(n) = q.get("name").cloned() else {
        return error(StatusCode::BAD_REQUEST);
    };
    let directory = match q.get("directory").map(String::as_str) {
        Some("true") if b.is_empty() => true,
        Some("false") => false,
        _ => return error(StatusCode::BAD_REQUEST),
    };
    if h.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/octet-stream") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Some(repo) = s.files.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    completed(
        tokio::task::spawn_blocking(move || {
            repo.create(u, &p, &n, if directory { None } else { Some(b.to_vec()) })
        })
        .await,
    )
}
async fn relocate(
    State(s): State<Arc<GatewayState>>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
    b: Bytes,
) -> Response {
    if !valid_origin(&h, &s.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let u = match user(&s, &h).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Some(id), Some(p), Some(n)) = (key(&q, "id"), key(&q, "parent"), q.get("name").cloned())
    else {
        return error(StatusCode::BAD_REQUEST);
    };
    if !b.is_empty() {
        return error(StatusCode::BAD_REQUEST);
    }
    let Some(repo) = s.files.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    completed(tokio::task::spawn_blocking(move || repo.relocate(u, &id, &p, &n)).await)
}
async fn remove(
    State(s): State<Arc<GatewayState>>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
) -> Response {
    if !valid_origin(&h, &s.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let u = match user(&s, &h).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(id) = key(&q, "id") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(repo) = s.files.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    completed(tokio::task::spawn_blocking(move || repo.delete(u, &id)).await)
}
async fn write(
    State(s): State<Arc<GatewayState>>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
    b: Bytes,
) -> Response {
    if !valid_origin(&h, &s.config.public_origin) {
        return error(StatusCode::FORBIDDEN);
    }
    let u = match user(&s, &h).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(id) = key(&q, "id") else {
        return error(StatusCode::BAD_REQUEST);
    };
    if h.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/octet-stream") {
        return error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Some(repo) = s.files.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    completed(tokio::task::spawn_blocking(move || repo.write(u, &id, b.to_vec())).await)
}
async fn download(
    State(s): State<Arc<GatewayState>>,
    Query(q): Query<HashMap<String, String>>,
    h: HeaderMap,
) -> Response {
    let u = match user(&s, &h).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(id) = key(&q, "id") else {
        return error(StatusCode::BAD_REQUEST);
    };
    let Some(repo) = s.files.clone() else {
        return error(StatusCode::SERVICE_UNAVAILABLE);
    };
    match tokio::task::spawn_blocking(move || repo.read(u, &id)).await {
        Ok(Ok((name, data))) => {
            let encoded = name
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect::<String>();
            let mut r = data.into_response();
            secure_headers(&mut r);
            r.headers_mut()
                .insert("content-type", "application/octet-stream".parse().unwrap());
            r.headers_mut().insert(
                "content-disposition",
                format!("attachment; filename=\"download\"; filename*=UTF-8''{encoded}")
                    .parse()
                    .unwrap(),
            );
            r
        }
        Ok(Err(e)) => fail(e),
        _ => error(StatusCode::SERVICE_UNAVAILABLE),
    }
}
