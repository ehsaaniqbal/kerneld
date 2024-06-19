use super::ApiResponse;
use crate::{
    kernel_manager::{Kernel, ReportId},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct LaunchKernelRequest {
    report_id: ReportId,
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/kernels", get(list_kernels))
        .route("/kernels/:report_id", get(get_kernel))
        .route("/kernels", post(launch_kernel))
        .route("/kernels/:report_id", delete(kill_kernel))
        .route("/kernels/:report_id/restart", post(restart_kernel))
}

async fn list_kernels(State(state): State<AppState>) -> impl IntoResponse {
    let kernels = state.kernel_manager.lock().await.get_kernels().await;
    Json(ApiResponse {
        success: true,
        data: Some(kernels.values().cloned().collect::<Vec<Kernel>>()),
        error: None,
    })
}

async fn get_kernel(
    State(state): State<AppState>,
    Path(report_id): Path<ReportId>,
) -> impl IntoResponse {
    match state
        .kernel_manager
        .lock()
        .await
        .get_kernel(report_id.clone())
        .await
    {
        Some(kernel) => (StatusCode::OK, Json(ApiResponse::success(kernel))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Kernel not found")),
        )
            .into_response(),
    }
}

async fn launch_kernel(
    State(state): State<AppState>,
    Json(payload): Json<LaunchKernelRequest>,
) -> impl IntoResponse {
    match state
        .kernel_manager
        .lock()
        .await
        .launch(payload.report_id)
        .await
    {
        Ok(kernel) => (StatusCode::OK, Json(ApiResponse::success(kernel))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(&e)),
        )
            .into_response(),
    }
}

async fn kill_kernel(
    State(state): State<AppState>,
    Path(report_id): Path<ReportId>,
) -> impl IntoResponse {
    match state.kernel_manager.lock().await.kill(report_id).await {
        Ok(_) => (StatusCode::OK, Json(ApiResponse::success(true))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(&e)),
        )
            .into_response(),
    }
}

async fn restart_kernel(
    State(state): State<AppState>,
    Path(report_id): Path<ReportId>,
) -> impl IntoResponse {
    match state.kernel_manager.lock().await.restart(report_id).await {
        Ok(kernel) => (StatusCode::OK, Json(ApiResponse::success(kernel))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(&e)),
        )
            .into_response(),
    }
}
