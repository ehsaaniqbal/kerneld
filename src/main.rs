use axum::http::StatusCode;
use axum::{routing::get, Router};
use kernel_gateway::kernel_manager::KernelManager;
use kernel_gateway::{routes, AppState};
use std::sync::Arc;
use std::{env, net::SocketAddr};
use tokio::sync::Mutex;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let host = env::var("KERNEL_GATEWAY_HOST").unwrap_or("::".to_string());
    let port = env::var("KERNEL_GATEWAY_PORT").unwrap_or("1111".to_string());

    let state = AppState {
        kernel_manager: Arc::new(Mutex::new(KernelManager::new())),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .merge(
            Router::new().nest(
                "/v1",
                Router::new()
                    .merge(routes::kernel::create_router())
                    .merge(routes::sysinfo::create_router()),
            ),
        )
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
        .await
        .expect("Failed to bind to address");
    info!(
        "kernel-gateway listening on {:?}",
        listener.local_addr().expect("Failed to get local address")
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Failed to start kernel-gateway");
}

async fn health_handler() -> (StatusCode, &'static str) {
    (StatusCode::OK, "OK")
}
