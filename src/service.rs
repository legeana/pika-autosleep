use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
    ServiceType, SessionChangeParam,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

pub const SERVICE_NAME: &str = "pika-autosleep";
pub const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

pub fn start() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .with_context(|| format!("failed to start {SERVICE_NAME}"))?;
    Ok(())
}

fn service_main(args: Vec<OsString>) {
    if let Err(e) = service_main_with_result(args) {
        log::error!("{e}");
    }
}

fn service_main_with_result(_args: Vec<OsString>) -> Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let event_handler = move |event: ServiceControl| -> ServiceControlHandlerResult {
        match event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                shutdown_tx.send(()).expect("shutdown_tx.send");
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::SessionChange(session) => handle_session(session),
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .with_context(|| format!("failed to register event handler for {SERVICE_NAME}"))?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::SESSION_CHANGE | ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .with_context(|| format!("failed to set {SERVICE_NAME} status"))?;

    shutdown_rx.recv().context("failed to wait for shutdown")?;
    Ok(())
}

fn handle_session(_session: SessionChangeParam) -> ServiceControlHandlerResult {
    ServiceControlHandlerResult::NotImplemented
}
