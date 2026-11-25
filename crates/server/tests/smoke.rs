// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! Smoke tests for the HTTP surface: the router answered in process, and the
//! `--healthcheck` probe answered by a real listener.

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::response::Response;
use tower::ServiceExt;
use trafficlights_server::{build_router, healthcheck};

async fn get(uri: &str) -> Response {
    build_router()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response")
}

async fn body_text(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
async fn health_reports_ok_as_json() {
    let response = get("/api/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content type"),
        "application/json"
    );
    assert_eq!(body_text(response).await, r#"{"status":"ok"}"#);
}

#[tokio::test]
async fn embedded_app_is_served_for_the_root_and_for_client_routes() {
    for uri in ["/", "/new", "/s/abc123"] {
        let response = get(uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .expect("content type"),
            "text/html; charset=utf-8",
            "{uri}"
        );
        assert!(body_text(response).await.contains("<html"), "{uri}");
    }
}

#[tokio::test]
async fn unknown_api_paths_and_missing_files_are_not_found() {
    for uri in ["/api/nope", "/api", "/assets/missing-0000.js"] {
        assert_eq!(get(uri).await.status(), StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn health_rejects_other_methods() {
    let response = build_router()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test(flavor = "multi_thread")]
async fn healthcheck_succeeds_against_a_running_server() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind an ephemeral port");
    let bind = listener.local_addr().expect("local address").to_string();
    let server = tokio::spawn(async move { axum::serve(listener, build_router()).await });

    let healthy = tokio::task::spawn_blocking(move || healthcheck(&bind))
        .await
        .expect("healthcheck task");
    server.abort();
    assert!(healthy);
}
