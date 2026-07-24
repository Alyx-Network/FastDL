use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwap;
use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use percent_encoding::percent_decode_str;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::config::Config;
use crate::directory::{generate_directory_listing, DirectoryEntry};
use crate::rules::{process_rules, Decision, Facts};

const DISABLED_EXTENSIONS: [&str; 3] = [".inc", ".sp", ".smx"];
const COMPRESSIONS: [&str; 2] = [".bz2", ".gz"];
const STREAM_CHUNK: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ArcSwap<Config>>,
    pub storage_root: PathBuf,
    pub auto_root: PathBuf,
}

pub async fn handle_request(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let started = Instant::now();
    let client = client_ip(&headers, remote);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let head = method == Method::HEAD;

    let url_path = percent_decode_str(uri.path()).decode_utf8_lossy().to_string();
    let display_path = match url_path.as_str() {
        "/" => String::new(),
        other => other.to_string(),
    };
    let extension = Path::new(&url_path)
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    let log_path = match display_path.is_empty() {
        true => "/".to_string(),
        false => display_path.clone(),
    };

    let target = match resolve_within(&state.storage_root, &url_path) {
        Some(target) => target,
        None => {
            tracing::warn!(client_ip = %client, path = %log_path, "Path escapes storage [path_escape_blocked]");
            log_access(&client, &method, &log_path, 403, 0, &user_agent, started.elapsed().as_millis());
            return respond(StatusCode::FORBIDDEN, "Forbidden");
        }
    };

    let config = state.config.load();

    let facts = Facts {
        path: display_path.clone(),
        user_agent: user_agent.clone(),
        method: method.to_string(),
        ext: extension.clone(),
        ip: client.clone(),
        peer_ip: normalize_ip(remote.ip().to_string()),
    };
    if let Decision::Deny { status, message, rule } = process_rules(&config.rules, &config.regexes, &facts, &headers) {
        let code = StatusCode::from_u16(status).unwrap_or(StatusCode::FORBIDDEN);
        tracing::warn!(client_ip = %client, path = %log_path, status, rule = %rule, reason = %message, "Blocked by rule [rule_blocked]");
        log_access(&client, &method, &log_path, status, 0, &user_agent, started.elapsed().as_millis());
        return (code, message).into_response();
    }

    if DISABLED_EXTENSIONS.contains(&extension.as_str()) {
        tracing::warn!(client_ip = %client, path = %log_path, extension = %extension, "Extension blocked [extension_blocked]");
        log_access(&client, &method, &log_path, 403, 0, &user_agent, started.elapsed().as_millis());
        return respond(StatusCode::FORBIDDEN, "Forbidden");
    }

    match tokio::fs::metadata(&target).await {
        Ok(metadata) if metadata.is_dir() => {
            return serve_directory(&config, &target, &display_path, &log_path, &client, &method, &user_agent, started).await;
        }
        Ok(metadata) if metadata.is_file() => {
            return serve_file(&target, metadata.len(), &headers, &client, &method, &log_path, &user_agent, head, started).await;
        }
        _ => {}
    }

    for compression in COMPRESSIONS {
        let candidate = append_extension(&target, compression);
        if let Some(length) = file_size(&candidate).await {
            return serve_file(&candidate, length, &headers, &client, &method, &log_path, &user_agent, head, started).await;
        }
    }

    if let Some(candidate) = resolve_within(&state.auto_root, &url_path) {
        if let Some(length) = file_size(&candidate).await {
            return serve_file(&candidate, length, &headers, &client, &method, &log_path, &user_agent, head, started).await;
        }
        for compression in COMPRESSIONS {
            let variant = append_extension(&candidate, compression);
            if let Some(length) = file_size(&variant).await {
                return serve_file(&variant, length, &headers, &client, &method, &log_path, &user_agent, head, started).await;
            }
        }
    }

    tracing::warn!(client_ip = %client, path = %log_path, "File not found [file_not_found]");
    log_access(&client, &method, &log_path, 404, 0, &user_agent, started.elapsed().as_millis());
    respond(StatusCode::NOT_FOUND, "Not Found")
}

async fn serve_directory(
    config: &Config,
    path: &Path,
    display_path: &str,
    log_path: &str,
    client: &str,
    method: &Method,
    user_agent: &str,
    started: Instant,
) -> Response {
    let directory = &config.directory_listing;
    let location = match display_path.is_empty() {
        true => "/",
        false => display_path,
    };
    let allowed = match directory.enabled {
        false => false,
        true => {
            directory.allowed_paths.is_empty()
                || directory
                    .allowed_paths
                    .iter()
                    .any(|entry| location == entry || location.starts_with(&format!("{entry}/")))
        }
    };
    if !allowed {
        tracing::warn!(client_ip = %client, path = %log_path, "Directory listing disabled [listing_disabled]");
        log_access(client, method, log_path, 403, 0, user_agent, started.elapsed().as_millis());
        return respond(StatusCode::FORBIDDEN, "Forbidden");
    }

    let items = match read_directory(path).await {
        Ok(items) => items,
        Err(error) => {
            tracing::error!(client_ip = %client, path = %log_path, error = %error, "Directory listing error [listing_failed]");
            log_access(client, method, log_path, 500, 0, user_agent, started.elapsed().as_millis());
            return respond(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
        }
    };

    let current = match display_path.is_empty() {
        true => "/".to_string(),
        false => display_path.to_string(),
    };
    let body = generate_directory_listing(&items, &current, &parent_path(display_path));
    let length = body.len() as u64;
    let mut response = (StatusCode::OK, body).into_response();
    let map = response.headers_mut();
    map.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    map.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache, no-store, must-revalidate"));
    map.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    map.insert(header::EXPIRES, HeaderValue::from_static("0"));
    log_access(client, method, log_path, 200, length, user_agent, started.elapsed().as_millis());
    response
}

async fn serve_file(
    path: &Path,
    size: u64,
    headers: &HeaderMap,
    client: &str,
    method: &Method,
    log_path: &str,
    user_agent: &str,
    head: bool,
    started: Instant,
) -> Response {
    let content_type = mime_guess::from_path(path).first_or_octet_stream().to_string();
    let range = match headers.get(header::RANGE).and_then(|value| value.to_str().ok()) {
        Some(value) => match parse_range(value, size) {
            Some(range) => Some(range),
            None => {
                log_access(client, method, log_path, 416, 0, user_agent, started.elapsed().as_millis());
                return range_not_satisfiable(size);
            }
        },
        None => None,
    };

    match build_file_response(path, size, range, &content_type, head).await {
        Ok((response, length)) => {
            let status = match range {
                Some(_) => 206,
                None => 200,
            };
            log_access(client, method, log_path, status, length, user_agent, started.elapsed().as_millis());
            response
        }
        Err(error) => {
            tracing::error!(client_ip = %client, path = %log_path, error = %error, "Failed to read file [file_read_failed]");
            log_access(client, method, log_path, 500, 0, user_agent, started.elapsed().as_millis());
            respond(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
        }
    }
}

async fn build_file_response(
    path: &Path,
    size: u64,
    range: Option<(u64, u64)>,
    content_type: &str,
    head: bool,
) -> std::io::Result<(Response, u64)> {
    let (status, start, length) = match range {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end - start + 1),
        None => (StatusCode::OK, 0, size),
    };

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_LENGTH, length)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0");
    if let Some((start, end)) = range {
        builder = builder.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }

    if head {
        let response = builder
            .body(Body::empty())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        return Ok((response, length));
    }

    let mut file = File::open(path).await?;
    if start > 0 {
        file.seek(std::io::SeekFrom::Start(start)).await?;
    }
    let stream = ReaderStream::with_capacity(file.take(length), STREAM_CHUNK);
    let response = builder
        .body(Body::from_stream(stream))
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
    Ok((response, length))
}

async fn read_directory(path: &Path) -> std::io::Result<Vec<DirectoryEntry>> {
    let mut reader = tokio::fs::read_dir(path).await?;
    let mut items = Vec::new();
    while let Some(entry) = reader.next_entry().await? {
        let metadata = entry.metadata().await?;
        let is_directory = metadata.is_dir();
        items.push(DirectoryEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            is_directory,
            size: match is_directory {
                true => None,
                false => Some(metadata.len()),
            },
        });
    }
    items.sort_by(|first, second| match (first.is_directory, second.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => first.name.cmp(&second.name),
    });
    Ok(items)
}

async fn file_size(path: &Path) -> Option<u64> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => Some(metadata.len()),
        _ => None,
    }
}

fn append_extension(path: &Path, extension: &str) -> PathBuf {
    let mut value = path.to_path_buf().into_os_string();
    value.push(extension);
    PathBuf::from(value)
}

fn resolve_within(storage_root: &Path, url_path: &str) -> Option<PathBuf> {
    let candidate = storage_root.join(url_path.trim_start_matches('/'));
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    match normalized.starts_with(storage_root) {
        true => Some(normalized),
        false => None,
    }
}

fn parent_path(display_path: &str) -> String {
    let trimmed = display_path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => trimmed[..index].to_string(),
    }
}

fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let (start_text, end_text) = spec.split_once('-')?;
    if start_text.is_empty() && end_text.is_empty() {
        return None;
    }
    let start = match start_text.is_empty() {
        true => 0,
        false => start_text.parse::<u64>().ok()?,
    };
    let end = match end_text.is_empty() {
        true => size.saturating_sub(1),
        false => end_text.parse::<u64>().ok()?.min(size.saturating_sub(1)),
    };
    if start > end || start >= size {
        return None;
    }
    Some((start, end))
}

fn range_not_satisfiable(size: u64) -> Response {
    let mut response = (StatusCode::RANGE_NOT_SATISFIABLE, "Requested Range Not Satisfiable").into_response();
    if let Ok(value) = HeaderValue::from_str(&format!("bytes */{size}")) {
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    response
}

fn respond(status: StatusCode, body: &'static str) -> Response {
    (status, body).into_response()
}

fn client_ip(headers: &HeaderMap, remote: SocketAddr) -> String {
    let read = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    if let Some(value) = read("cf-connecting-ip") {
        return normalize_ip(value);
    }
    if let Some(value) = read("x-real-ip") {
        return normalize_ip(value);
    }
    if let Some(value) = read("x-forwarded-for") {
        if let Some(first) = value.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return normalize_ip(trimmed.to_string());
            }
        }
    }
    normalize_ip(remote.ip().to_string())
}

fn normalize_ip(value: String) -> String {
    if value == "::1" {
        return "127.0.0.1".to_string();
    }
    match value.strip_prefix("::ffff:") {
        Some(stripped) => stripped.to_string(),
        None => value,
    }
}

fn log_access(
    client: &str,
    method: &Method,
    path: &str,
    status: u16,
    bytes: u64,
    user_agent: &str,
    elapsed_ms: u128,
) {
    tracing::info!(
        client_ip = client,
        method = %method,
        path,
        status,
        bytes,
        user_agent,
        elapsed_ms,
        "Request served [request_served]"
    );
}
