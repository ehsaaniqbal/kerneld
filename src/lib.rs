pub mod kernel_manager;
pub mod routes;

use std::sync::Arc;
use tokio::sync::Mutex;

use kernel_manager::KernelManager;

#[derive(Clone)]
pub struct AppState {
    pub kernel_manager: Arc<Mutex<KernelManager>>,
}
