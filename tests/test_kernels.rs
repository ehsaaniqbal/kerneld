use kernel_gateway::kernel_manager::KernelManager;
use std::{
    process::{Command, Stdio},
    thread::sleep,
    time::Duration,
};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_start_kernel() {
    let mut kernel_manager = KernelManager::new();
    let kernel = kernel_manager.launch("test_report".to_string()).await;
    dbg!("{:?}", kernel);
    dbg!("{:?}", &kernel_manager);
}

// #[tokio::test]
// async fn test_kill_kernel() {
//     let mut kernel_manager = KernelManager::new();
//     let kernel = kernel_manager.start("test_report".to_string()).await;
//     dbg!("{:?}", kernel);
//     dbg!("{:?}", &kernel_manager);
//     kernel_manager.kill(kernel).await;
//     dbg!("{:?}", &kernel_manager);
// }

#[tokio::test]
async fn test_port_available() {
    let port = 8080; // replace with the port you want to check
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await;

    match listener {
        Ok(_) => println!("Port {} is available", port),
        Err(_) => println!("Port {} is not available", port),
    }
}
