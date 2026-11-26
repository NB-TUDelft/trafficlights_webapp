// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! The trafficlights-webapp server, as described in "Architecture and
//! design" in the knowledge base:
//!
//! - `GET /api/health` for the container health check;
//! - `GET /api/sessions/{id}/log`, the creator-authorized streaming
//!   download of a session's statistics `.jsonl` file;
//! - `WS /ws/lobby` and `WS /ws/sessions/{id}` (see [`ws`]), backed by the
//!   in-memory session registry in [`AppState`];
//! - the single-page app embedded from `web/dist` for every other `GET`,
//!   with the optional `CUSTOM_SCRIPT_SRC` script added to its head.
//!
//! [`build_router_with_state`] wires these to one [`AppState`];
//! [`AppState::shutdown`] ends every session cleanly when the server stops.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde_json::json;
use tokio_util::io::ReaderStream;

mod board;
mod client;
pub mod config;
mod log;
mod session;
mod state;
pub mod ws;

pub use config::{ConfigError, CustomScript, ServerConfig};
pub use state::AppState;

/// Read-buffer size for streaming a statistics log download.
const LOG_STREAM_CHUNK_BYTES: usize = 16 * 1024;

/// The built frontend, baked into the binary in release builds (and read from
/// disk in debug builds). `build.rs` guarantees the folder exists.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct WebAssets;

/// Where the project's source code is published, for AGPL Section 13.
pub const SOURCE_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// The project's license, for `--version` and the AGPL Section 13 notice.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// The address the server binds to: `BIND`, defaulting to `127.0.0.1:3000`
/// when unset or blank (the same rule as [`ServerConfig::from_env`]).
pub fn bind_address() -> String {
    std::env::var("BIND")
        .ok()
        .map(|bind| bind.trim().to_string())
        .filter(|bind| !bind.is_empty())
        .unwrap_or_else(|| config::DEFAULT_BIND.to_string())
}

/// The commit this binary was built from, when the image build passes it in
/// `TRAFFICLIGHTS_COMMIT`; absent in a local development build.
pub fn build_commit() -> Option<&'static str> {
    option_env!("TRAFFICLIGHTS_COMMIT").filter(|commit| !commit.trim().is_empty())
}

/// What `trafficlights-webapp --version` prints: the version (with the short
/// commit when known), the license and the link to the Corresponding Source.
pub fn version_text() -> String {
    format_version(env!("CARGO_PKG_VERSION"), build_commit())
}

fn format_version(version: &str, commit: Option<&str>) -> String {
    let stamp = match commit {
        Some(commit) => {
            let short: String = commit.trim().chars().take(7).collect();
            format!("{version} ({short})")
        }
        None => version.to_string(),
    };
    format!("trafficlights-webapp {stamp}\nLicense: {LICENSE}\nSource: {SOURCE_URL}")
}

/// The application router with a default configuration and fresh state;
/// see [`build_router_with_state`].
pub fn build_router() -> Router {
    build_router_with_state(Arc::new(AppState::new(ServerConfig::default())))
}

/// The application router over `state`: `GET /api/health`,
/// `GET /api/sessions/{id}/log`, `WS /ws/lobby`, `WS /ws/sessions/{id}`,
/// plus the embedded single-page app for every other `GET`.
pub fn build_router_with_state(state: Arc<AppState>) -> Router {
    let page = Arc::new(Page::new(state.config().custom_script.as_ref()));
    Router::new()
        .route("/api/health", get(health))
        .route("/api/sessions/{id}/log", get(session_log))
        .route("/ws/lobby", get(ws::lobby))
        .route("/ws/sessions/{id}", get(ws::session))
        .fallback_service(get(move |uri: Uri| {
            let page = Arc::clone(&page);
            async move { static_fallback(&page, &uri) }
        }))
        .with_state(state)
}

/// How the embedded single-page app is served: its
/// Content-Security-Policy, and the custom `<script>` element added to the
/// head of `index.html` when `CUSTOM_SCRIPT_SRC` is set. Built once at
/// startup.
#[derive(Debug)]
struct Page {
    csp: String,
    head_script: Option<String>,
}

impl Page {
    fn new(custom: Option<&CustomScript>) -> Self {
        Self {
            csp: content_security_policy(custom),
            head_script: custom.map(script_element),
        }
    }
}

/// The Content-Security-Policy of the embedded app. A custom script's
/// origin is allowed for scripts, connections, images, and styles.
fn content_security_policy(custom: Option<&CustomScript>) -> String {
    let extra = custom
        .map(|script| format!(" {}", script.origin()))
        .unwrap_or_default();
    format!(
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'{extra}; connect-src 'self' ws: wss:{extra}; img-src 'self' data:{extra}; style-src 'self'{extra}; frame-ancestors 'none'; base-uri 'none'"
    )
}

/// The deferred `<script>` element of a custom script.
fn script_element(script: &CustomScript) -> String {
    format!(
        "<script src=\"{}\" defer></script>",
        escape_attribute(script.src())
    )
}

/// `text` escaped for a double-quoted HTML attribute value.
fn escape_attribute(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// `html` with `element` inserted right before its `</head>`, or `None`
/// when the document is not UTF-8 or has no `</head>`.
fn inject_into_head(html: &[u8], element: &str) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(html).ok()?;
    let at = text.find("</head>")?;
    let mut out = String::with_capacity(text.len() + element.len());
    out.push_str(&text[..at]);
    out.push_str(element);
    out.push_str(&text[at..]);
    Some(out.into_bytes())
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

/// A JSON error response: `{"error": {"code": ..., "message": ...}}`.
pub(crate) fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

/// The credentials of an `Authorization: Bearer <token>` header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
}

/// `GET /api/sessions/{id}/log`: stream a live or recently ended session's
/// statistics `.jsonl` file to its creator.
///
/// The ID must look like a session ID, and the file path always comes from
/// the registry, never from the request, so no request can reach another
/// file. Unknown or expired sessions are `404`; a missing or wrong
/// `Authorization: Bearer <creator_token>` is `401` (compared in constant
/// time). The file is streamed in fixed-size chunks, so a download costs the
/// same small amount of memory however long the class ran (R15).
async fn session_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let not_found = || api_error(StatusCode::NOT_FOUND, "not_found", "Session log not found.");
    if !ws::valid_session_id(&id) {
        return not_found();
    }
    let Some((token, path)) = state.log_access(&id) else {
        return not_found();
    };
    if !bearer_token(&headers).is_some_and(|provided| token.matches(provided)) {
        let mut response = api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "The session's creator token is required.",
        );
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
        return response;
    }
    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return not_found(),
        Err(error) => {
            tracing::warn!(session_id = %id, %error, "cannot open a session log");
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The session log could not be read.",
            );
        }
    };
    let body = Body::from_stream(ReaderStream::with_capacity(file, LOG_STREAM_CHUNK_BYTES));
    (
        [
            (
                header::CONTENT_TYPE,
                "application/x-ndjson; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"trafficlights-{id}.jsonl\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        body,
    )
        .into_response()
}

/// Embedded files by exact path; any path without a file extension is a
/// client-side route and gets `index.html`. Unknown `/api/...` and `/ws/...`
/// paths and missing files are real 404s, so a stale asset never turns into
/// HTML.
fn static_fallback(page: &Page, uri: &Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let reserved = ["api", "ws"]
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")));
    if reserved {
        return api_error(StatusCode::NOT_FOUND, "not_found", "not found");
    }
    if let Some(response) = embedded(page, path) {
        return response;
    }
    if !has_extension(path) {
        if let Some(response) = embedded(page, "index.html") {
            return response;
        }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn embedded(page: &Page, path: &str) -> Option<Response> {
    let file = WebAssets::get(path)?;
    let mut body = file.data.into_owned();
    if path == "index.html" {
        if let Some(element) = &page.head_script {
            match inject_into_head(&body, element) {
                Some(injected) => body = injected,
                None => tracing::warn!("index.html has no </head>; custom script not added"),
            }
        }
    }
    Some(
        (
            [
                (header::CONTENT_TYPE, content_type(path)),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
                (header::REFERRER_POLICY, "no-referrer".to_string()),
                (header::X_FRAME_OPTIONS, "DENY".to_string()),
                (header::CONTENT_SECURITY_POLICY, page.csp.clone()),
            ],
            body,
        )
            .into_response(),
    )
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

/// The `Content-Type` for an embedded file. Browsers refuse to stream-compile
/// WebAssembly unless it is served as `application/wasm`.
fn content_type(path: &str) -> String {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    if mime.type_() == mime_guess::mime::TEXT {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

/// The TCP port of a `BIND` value such as `0.0.0.0:3000` or `[::]:3000`.
fn bind_port(bind: &str) -> Option<u16> {
    bind.rsplit_once(':')?.1.parse().ok()
}

/// Probe this server's own `/api/health` on the loopback interface, for the
/// container's `HEALTHCHECK` (the distroless image has no `curl` or `wget`).
/// `bind` is the server's `BIND` value; only its port is used.
pub fn healthcheck(bind: &str) -> bool {
    let Some(port) = bind_port(bind) else {
        return false;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let timeout = Duration::from_secs(3);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
        return false;
    };
    if stream.set_read_timeout(Some(timeout)).is_err()
        || stream.set_write_timeout(Some(timeout)).is_err()
    {
        return false;
    }
    let request =
        format!("GET /api/health HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    response.starts_with("HTTP/1.1 200") && response.contains(r#""status":"ok""#)
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};

    use super::{
        bearer_token, bind_port, content_security_policy, content_type, escape_attribute,
        format_version, has_extension, inject_into_head, script_element, CustomScript, LICENSE,
        SOURCE_URL,
    };

    fn script(src: &str) -> CustomScript {
        CustomScript::parse(src).expect("a valid custom script")
    }

    #[test]
    fn the_csp_allows_a_custom_script_origin_only_when_configured() {
        assert_eq!(
            content_security_policy(None),
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' ws: wss:; img-src 'self' data:; style-src 'self'; frame-ancestors 'none'; base-uri 'none'"
        );
        let custom = script("https://Scripts.Example.com/s.js?site=1");
        assert_eq!(
            content_security_policy(Some(&custom)),
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' https://scripts.example.com; connect-src 'self' ws: wss: https://scripts.example.com; img-src 'self' data: https://scripts.example.com; style-src 'self' https://scripts.example.com; frame-ancestors 'none'; base-uri 'none'"
        );
    }

    #[test]
    fn the_custom_script_element_is_deferred_and_escaped() {
        assert_eq!(
            script_element(&script("https://cdn.example.com/s.js?a=1&b=2")),
            "<script src=\"https://cdn.example.com/s.js?a=1&amp;b=2\" defer></script>"
        );
        assert_eq!(
            escape_attribute("a&b\"c'd<e>f"),
            "a&amp;b&quot;c&#39;d&lt;e&gt;f"
        );
    }

    #[test]
    fn scripts_are_injected_right_before_the_head_closes() {
        let html = b"<html><head><title>x</title></head><body></body></html>";
        assert_eq!(
            inject_into_head(html, "<script></script>").as_deref(),
            Some(&b"<html><head><title>x</title><script></script></head><body></body></html>"[..])
        );
        assert_eq!(inject_into_head(b"<html><body></body></html>", "<s>"), None);
        assert_eq!(inject_into_head(&[0xff, 0xfe], "<s>"), None);
    }

    fn authorization(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn bearer_tokens_are_read_from_the_authorization_header() {
        assert_eq!(bearer_token(&authorization("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(&authorization("bearer  abc ")), Some("abc"));
        assert_eq!(bearer_token(&authorization("Basic abc")), None);
        assert_eq!(bearer_token(&authorization("Bearer")), None);
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }

    #[test]
    fn version_names_the_license_and_the_source() {
        let text = format_version("1.2.3", None);
        assert_eq!(
            text,
            format!("trafficlights-webapp 1.2.3\nLicense: {LICENSE}\nSource: {SOURCE_URL}")
        );
        assert_eq!(LICENSE, "AGPL-3.0-or-later");
        assert_eq!(
            SOURCE_URL,
            "https://github.com/NB-TUDelft/trafficlights_webapp"
        );
    }

    #[test]
    fn version_shortens_the_commit_to_seven_characters() {
        let text = format_version("1.2.3", Some("0123456789abcdef\n"));
        assert!(text.starts_with("trafficlights-webapp 1.2.3 (0123456)\n"));
    }

    #[test]
    fn bind_port_reads_ipv4_and_ipv6_binds() {
        assert_eq!(bind_port("0.0.0.0:3000"), Some(3000));
        assert_eq!(bind_port("127.0.0.1:8080"), Some(8080));
        assert_eq!(bind_port("[::]:3000"), Some(3000));
        assert_eq!(bind_port("3000"), None);
        assert_eq!(bind_port("host:notaport"), None);
    }

    #[test]
    fn web_assets_get_the_content_types_browsers_need() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("assets/app.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("icon.svg"), "image/svg+xml");
        assert_eq!(
            content_type("assets/trafficlights_wasm_bg.wasm"),
            "application/wasm"
        );
        assert_eq!(
            content_type("manifest.webmanifest"),
            "application/manifest+json"
        );
        assert!(content_type("assets/app.js").starts_with("text/javascript"));
    }

    #[test]
    fn only_paths_with_a_file_name_extension_are_files() {
        assert!(has_extension("assets/app.js"));
        assert!(has_extension("icon.svg"));
        assert!(!has_extension(""));
        assert!(!has_extension("s/abc123"));
        assert!(!has_extension("v1.2/session"));
    }
}
