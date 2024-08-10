use std::ffi::OsString;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{select_biased, tick, unbounded};
use windows_service::service::{
    PowerEventParam, ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState,
    ServiceStatus, ServiceType, SessionChangeReason,
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
    let (shutdown_tx, shutdown_rx) = unbounded();
    let (power_tx, power_rx) = unbounded();
    let (session_tx, session_rx) = unbounded();
    let ticker = tick(Duration::from_secs(60));

    let events = ServiceControlAccept::SESSION_CHANGE
        | ServiceControlAccept::STOP
        | ServiceControlAccept::POWER_EVENT;
    let event_handler = move |event: ServiceControl| -> ServiceControlHandlerResult {
        match event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                shutdown_tx.send(()).expect("shutdown_tx.send");
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::SessionChange(msg) => {
                session_tx.send(msg).expect("session_tx.send");
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::PowerEvent(msg) => {
                power_tx.send(msg).expect("power_tx.send");
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .with_context(|| format!("failed to register event handler for {SERVICE_NAME}"))?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: events,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .with_context(|| format!("failed to set {SERVICE_NAME} status"))?;

    let mut can_suspend: Option<Instant> = None;
    loop {
        select_biased! {
            // Always try to shut down ASAP.
            recv(shutdown_rx) -> msg => {
                msg.context("failed to wait for shutdown")?;
                return Ok(());
            }

            // Always prioritise actual events.
            recv(power_rx) -> msg => {
                let power_event = msg.context("failed to read POWER_EVENT")?;
                let schedule_suspend = matches!(
                    power_event,
                    PowerEventParam::ResumeAutomatic | PowerEventParam::ResumeCritical);
                if schedule_suspend {
                    can_suspend = Some(Instant::now() + Duration::from_secs(300));
                }
            }
            recv(session_rx) -> msg => {
                let session_change = msg.context("failed to read SESSION_CHANGE")?;
                let cancel_suspend = matches!(
                    session_change.reason,
                    SessionChangeReason::ConsoleConnect
                    | SessionChangeReason::RemoteConnect
                    | SessionChangeReason::SessionLogon
                    | SessionChangeReason::SessionUnlock
                    | SessionChangeReason::SessionRemoteControl
                    | SessionChangeReason::SessionCreate);
                if cancel_suspend {
                    can_suspend = None;
                }
            }

            // And handle ticks after everything else.
            recv(ticker) -> msg => {
                msg.context("failed to read ticker")?;
                if let Some(suspend_time) = can_suspend {
                    if suspend_time < Instant::now() {
                        if let Err(err) = suspend() {
                            log::error!("failed to suspend: {err}");
                        }
                    }
                }
            }
        };
    }
}

fn suspend() -> Result<()> {
    let ok = unsafe { windows::Win32::System::Power::SetSuspendState(false, true, true) };
    ok.ok().context("failed to SetSuspendState")
}
