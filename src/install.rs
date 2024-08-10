use std::ffi::OsString;

use anyhow::{Context, Result};
use windows_service::service::{
    ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::{cli, service};

const SERVICE_NAME: &str = service::SERVICE_NAME;

pub fn install() -> Result<()> {
    let binary = std::env::current_exe().context("failed to get current executable")?;
    let description = "Automatically puts computer to sleep if it's locked for a period of time";
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from("Pika AutoSleep"),
        service_type: service::SERVICE_TYPE,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: binary,
        launch_arguments: vec![OsString::from(cli::SERVICE_COMMAND)],
        dependencies: vec![],
        account_name: None, // Run as System.
        account_password: None,
    };

    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .context("ServiceManager::local_computer")?;
    let service = manager
        .create_service(&info, ServiceAccess::CHANGE_CONFIG)
        .context("ServiceManager::create_service")?;
    service
        .set_description(description)
        .context("Service::set_description")?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let manager = ServiceManager::local_computer(None::<&str>, manager_access)
        .context("ServiceManager::local_computer")?;
    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = manager
        .open_service(SERVICE_NAME, service_access)
        .with_context(|| format!("failed to open service {SERVICE_NAME}"))?;

    service
        .delete()
        .with_context(|| format!("failed to mark {SERVICE_NAME} for deletion"))?;

    if service
        .query_status()
        .with_context(|| format!("failed to query {SERVICE_NAME} status"))?
        .current_state
        != ServiceState::Stopped
    {
        service.stop().with_context(|| {
            format!("failed to stop {SERVICE_NAME}, it will be deleted when the system restarts")
        })?;
    }
    Ok(())
}
