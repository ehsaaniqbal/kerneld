use std::collections::HashMap;

use super::ApiResponse;
use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};

pub fn create_router() -> Router<AppState> {
    Router::new().route("/sysinfo", get(get_sysinfo))
}

async fn get_sysinfo(State(state): State<AppState>) -> impl IntoResponse {
    let kernel_manager = state.kernel_manager.lock().await;
    let mut system = kernel_manager.system.lock().await;
    system.refresh_memory();

    let mut memory = HashMap::new();
    memory.insert("total", system.total_memory());
    memory.insert("used", system.used_memory());

    let mut system_info = HashMap::new();
    system_info.insert("memory", memory);

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some(system_info),
            error: None,
        }),
    )
}
