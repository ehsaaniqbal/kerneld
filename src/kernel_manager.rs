use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Stdio, sync::Arc};
use sysinfo::{Pid, ProcessStatus, System};
use tokio::{
    net::TcpListener,
    process::Command,
    sync::Mutex,
    time::{sleep, Duration},
};
use tracing::debug;
use uuid::Uuid;

// Available port range
const PORT_RANGE: (u32, u32) = (2000, 65000);

pub type ReportId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents the status of a kernel.
pub enum KernelStatus {
    Running,
    Stopped,
    Error,
    Created,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Configuration settings for a ipykernel, including IP and allocated ports.
pub struct Config {
    pub ip: String,
    pub hb_port: u32,
    pub control_port: u32,
    pub shell_port: u32,
    pub iopub_port: u32,
    pub stdin_port: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents a kernel with its ID, process ID, configuration, status, and associated report ID.
pub struct Kernel {
    pub id: String,
    pub process_id: Option<u32>,
    pub config: Option<Config>,
    pub status: KernelStatus,
    pub report_id: ReportId,
}

impl Kernel {
    /// Creates a new `Kernel` instance with a given report ID.
    pub fn new(report_id: ReportId) -> Self {
        Kernel {
            id: Uuid::new_v4().to_string(),
            process_id: None,
            config: None,
            status: KernelStatus::Created,
            report_id,
        }
    }
}

#[derive(Debug, Clone)]
/// Manages kernels, including launching, killing, and retrieving kernel information.
pub struct KernelManager {
    kernels: Arc<Mutex<HashMap<ReportId, Kernel>>>, // Map of active kernels
    available_ports: Vec<u32>,                      // Pool of available port segments
    pub system: Arc<Mutex<System>>, // System info (ref: https://docs.rs/sysinfo/latest/sysinfo/struct.System.html)
}

impl KernelManager {
    /// Creates a new `KernelManager` instance.
    pub fn new() -> Self {
        KernelManager {
            kernels: Arc::new(Mutex::new(HashMap::new())),
            available_ports: (PORT_RANGE.0..PORT_RANGE.1).step_by(5).collect(),
            system: Arc::new(Mutex::new(System::new_all())),
        }
    }

    /// Retrieves a map of active kernels.
    pub async fn get_kernels(&self) -> HashMap<ReportId, Kernel> {
        self.kernels.lock().await.clone()
    }

    /// Retrieves a specific kernel by its report ID.
    pub async fn get_kernel(&self, report_id: ReportId) -> Option<Kernel> {
        self.kernels.lock().await.get(&report_id).cloned()
    }

    /// Launches a new kernel for the specified report ID.
    pub async fn launch(&mut self, report_id: ReportId) -> Result<Kernel, String> {
        let mut kernel = Kernel::new(report_id.clone());
        let Config {
            ip,
            hb_port,
            control_port,
            shell_port,
            iopub_port,
            stdin_port,
        } = self.get_config(report_id.clone()).await?;

        let python_executable_path =
            "/Users/ehsaan/miniconda3/envs/jupyter_server/bin/python".to_string(); // TODO: get from env
        let ipykernel_launcher_path = "ipykernel_launcher".to_string();
        let kernel_flags = "--debug --no-secure".to_string();

        let cmd = vec![
            python_executable_path.clone(),
            "-m".to_string(),
            ipykernel_launcher_path,
            kernel_flags,
            format!("--ip={}", ip),
            format!("--hb={}", hb_port),
            format!("--control={}", control_port),
            format!("--shell={}", shell_port),
            format!("--iopub={}", iopub_port),
            format!("--stdin={}", stdin_port),
        ];

        let kernel_process = Command::new(python_executable_path.clone())
            .args(&cmd[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match &kernel_process {
            Ok(process) => {
                let pid = process.id().expect("Failed to get kernel process id");
                debug!(
                    "Kernel for report {} spawned with PID: {}",
                    &report_id, &pid
                );

                // Wait for kernel to start
                sleep(Duration::from_millis(100)).await;

                // Check if kernel is running
                if self.is_process_running(pid.clone()).await {
                    kernel.process_id = Some(pid.clone());
                    kernel.status = KernelStatus::Running;
                    kernel.config = Some(Config {
                        ip,
                        hb_port,
                        control_port,
                        shell_port,
                        iopub_port,
                        stdin_port,
                    });
                    self.kernels
                        .lock()
                        .await
                        .insert(report_id.clone(), kernel.clone());
                    debug!(
                        "Kernel process {} is running with PID: {}",
                        &report_id, &pid
                    );
                    Ok(kernel)
                } else {
                    kernel.status = KernelStatus::Error;
                    Err(format!("Kernel process {} is not running", &report_id))
                }
            }
            Err(e) => Err(format!("Failed to start kernel: {}", e)),
        }
    }

    /// Kills a kernel process by its report ID.
    pub async fn kill(&mut self, report_id: ReportId) -> Result<bool, String> {
        let mut kernel_lock = self.kernels.lock().await;
        let kernel = kernel_lock.get(&report_id);
        if kernel.is_none() {
            return Err(format!("Kernel for report {} not found", &report_id));
        }

        let pid = kernel.unwrap().process_id.unwrap().to_string();
        let kill_process = Command::new("kill").arg(pid.clone()).output().await;

        match kill_process {
            Ok(_) => {
                kernel_lock.remove(&pid.to_string());
                // Add port segment back to available pool
                self.available_ports.push(pid.parse::<u32>().unwrap());
                debug!("Killed kernel with PID: {}", pid.clone());
                debug!("Available port segments: {:?}", &self.available_ports.len());
                Ok(true)
            }
            Err(e) => Err(format!("Failed to kill kernel: {}", e.to_string())),
        }
    }

    /// Retrieves a configuration with available ports for a kernel.
    /// Uses a pooling strategy to allocate available ports to avoid conflicts and fragmentation.
    async fn get_config(&mut self, report_id: ReportId) -> Result<Config, String> {
        let addr = "0.0.0.0".to_string(); // listen on all interfaces
        let mut config = Config {
            ip: addr.clone(),
            hb_port: 0,
            control_port: 0,
            shell_port: 0,
            iopub_port: 0,
            stdin_port: 0,
        };

        let mut current_pool_index = 0;
        let mut start_port = self
            .available_ports
            .first()
            .unwrap_or(&PORT_RANGE.0)
            .clone();
        let mut end_port = start_port + 5;
        let mut ports = (start_port..end_port).collect::<Vec<u32>>();

        loop {
            if self.are_ports_available(&addr, &ports).await {
                config.hb_port = ports[0];
                config.control_port = ports[1];
                config.shell_port = ports[2];
                config.iopub_port = ports[3];
                config.stdin_port = ports[4];
                // Remove allocated port segment from available pool
                self.available_ports.remove(current_pool_index);
                debug!("Available port segments: {:?}", &self.available_ports.len());
                break;
            } else {
                debug!(
                    "No available ports found for range: {}-{}. Trying again...",
                    &start_port, &end_port
                );
                // If no available ports are found, try again with a
                // random port segment from the available pool
                current_pool_index = rand::thread_rng().gen_range(1..self.available_ports.len());
                start_port = self.available_ports[current_pool_index];
                end_port = start_port + 5;
                ports = (start_port..end_port).collect::<Vec<u32>>();
            }
        }
        debug!("Allocated ports for report {}: {:?}", &report_id, &ports);
        Ok(config)
    }

    /// Restarts a kernel for the specified report ID.
    pub async fn restart(&mut self, report_id: ReportId) -> Result<Kernel, String> {
        // NOTE: there is no way to restart a process, so we kill the existing kernel and launch a new one
        self.kill(report_id.clone()).await?;
        self.launch(report_id).await
    }

    /// Checks if a range of ports are available for binding.
    async fn are_ports_available(&self, addr: &str, ports: &Vec<u32>) -> bool {
        for port in ports.iter() {
            let listener = TcpListener::bind(format!("{}:{}", addr, port)).await;
            match listener {
                Ok(_) => {}
                Err(e) => {
                    debug!("Port {} is not available. Error: {}", port, e);
                    return false;
                }
            }
        }
        true // all ports are available in the range
    }

    /// Checks if a process with the given PID is currently running.
    async fn is_process_running(&mut self, pid: u32) -> bool {
        let mut system = self.system.lock().await;
        if !system.refresh_process(Pid::from_u32(pid)) {
            debug!("Failed to refresh process info for PID: {}", pid);
            return false;
        }

        let process = system.process(Pid::from_u32(pid));
        match process {
            Some(p) => p.status() == ProcessStatus::Run,
            None => false,
        }
    }
}
