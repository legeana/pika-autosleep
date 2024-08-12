use std::ffi::OsString;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{select_biased, tick, unbounded};
use windows_service::service::{
    PowerEventParam, ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState,
    ServiceStatus, ServiceType, SessionChangeReason,
};
use windows_service::service_control_handler::{
    register, ServiceControlHandlerResult, ServiceStatusHandle,
};
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

pub const SERVICE_NAME: &str = "pika-autotools";
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
    log::warn!("finished service_main");
}

fn service_main_with_result(_args: Vec<OsString>) -> Result<()> {
    log::info!("started service");
    let (shutdown_tx, shutdown_rx) = unbounded();
    let (power_tx, power_rx) = unbounded();
    let (session_tx, session_rx) = unbounded();
    let ticker = tick(Duration::from_secs(60));
    log::info!("initialised channels");

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

    let status_handle = register(SERVICE_NAME, event_handler)
        .with_context(|| format!("failed to register event handler for {SERVICE_NAME}"))?;
    log::info!("registered event handler");

    set_state(&status_handle, ServiceState::Running, events)?;

    let mut can_suspend: Option<Instant> = None;
    loop {
        select_biased! {
            // Always try to shut down ASAP.
            recv(shutdown_rx) -> msg => {
                msg.context("failed to wait for shutdown")?;
                log::warn!("received shutdown request");
                set_state(&status_handle, ServiceState::Stopped, ServiceControlAccept::empty())?;
                return Ok(());
            }

            // Always prioritise actual events.
            recv(power_rx) -> msg => {
                let power_event = msg.context("failed to read POWER_EVENT")?;
                log::info!("received POWER_EVENT {power_event:?}");
                let schedule_suspend = matches!(
                    power_event,
                    PowerEventParam::ResumeAutomatic
                    | PowerEventParam::ResumeCritical
                    | PowerEventParam::ResumeSuspend);
                if schedule_suspend {
                    let login_timeout = Duration::from_secs(300);
                    let suspend_time = Instant::now() + login_timeout;
                    can_suspend = Some(suspend_time);
                    log::info!("scheduled suspend in {login_timeout:?}");
                }
            }
            recv(session_rx) -> msg => {
                let session_change = msg.context("failed to read SESSION_CHANGE")?;
                log::info!("received SESSION_CHANGE {session_change:?}");
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
                    log::info!("cancelled suspend");
                }
            }

            // And handle ticks after everything else.
            recv(ticker) -> msg => {
                msg.context("failed to read ticker")?;
                if let Some(suspend_time) = can_suspend {
                    let now = Instant::now();
                    if suspend_time < now {
                        log::info!("attempting to suspend in {:?}", suspend_time - now);
                        match suspend() {
                            Err(err) => log::error!("failed to suspend, will retry: {err}"),
                            Ok(()) => {
                                log::info!("suspend successful, resetting");
                                can_suspend = None;
                            }
                        }
                    }
                } else {
                    log::info!("no suspend is scheduled");
                }
            }
        };
    }
}

fn suspend() -> Result<()> {
    let ok = unsafe { windows::Win32::System::Power::SetSuspendState(false, true, true) };
    ok.ok().context("failed to SetSuspendState")
}

fn set_state(
    service: &ServiceStatusHandle,
    state: ServiceState,
    events: ServiceControlAccept,
) -> Result<()> {
    service
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: state,
            controls_accepted: events,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .with_context(|| format!("failed to set {SERVICE_NAME} status to {state:?}"))?;
    log::info!("set {SERVICE_NAME} to {state:?}");
    Ok(())
}
