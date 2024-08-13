use std::ffi::OsString;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows_service::service::{
    PowerEventParam, ServiceControlAccept, SessionChangeParam, SessionChangeReason,
};
use windows_service::{define_windows_service, service_dispatcher};

use crate::constants::SERVICE_NAME;
use crate::events;

define_windows_service!(ffi_service_main, service_main);

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

#[derive(Debug, Default)]
struct AutoSleeper {
    can_suspend: Option<Instant>,
}

impl events::Callbacks for AutoSleeper {
    fn tick_duration(&self) -> Duration {
        Duration::from_secs(60)
    }

    fn on_tick(&mut self, now: Instant) -> Result<()> {
        if let Some(suspend_time) = self.can_suspend {
            if suspend_time < now {
                log::info!("attempting to suspend in {:?}", suspend_time - now);
                match suspend() {
                    Err(err) => log::error!("failed to suspend, will retry: {err}"),
                    Ok(()) => {
                        log::info!("suspend successful, resetting");
                        self.can_suspend = None;
                    }
                }
            }
        } else {
            log::info!("no suspend is scheduled");
        }
        Ok(())
    }

    fn listen(&self) -> ServiceControlAccept {
        ServiceControlAccept::POWER_EVENT | ServiceControlAccept::SESSION_CHANGE
    }

    fn on_power_event(&mut self, power_event: PowerEventParam) -> Result<()> {
        log::info!("received POWER_EVENT {power_event:?}");
        let schedule_suspend = matches!(
            power_event,
            PowerEventParam::ResumeAutomatic
                | PowerEventParam::ResumeCritical
                | PowerEventParam::ResumeSuspend
        );
        if schedule_suspend {
            let login_timeout = Duration::from_secs(300);
            let suspend_time = Instant::now() + login_timeout;
            self.can_suspend = Some(suspend_time);
            log::info!("scheduled suspend in {login_timeout:?}");
        }
        Ok(())
    }

    fn on_session_change(&mut self, session_change: SessionChangeParam) -> Result<()> {
        log::info!("received SESSION_CHANGE {session_change:?}");
        let cancel_suspend = matches!(
            session_change.reason,
            SessionChangeReason::ConsoleConnect
                | SessionChangeReason::RemoteConnect
                | SessionChangeReason::SessionLogon
                | SessionChangeReason::SessionUnlock
                | SessionChangeReason::SessionRemoteControl
                | SessionChangeReason::SessionCreate
        );
        if cancel_suspend {
            self.can_suspend = None;
            log::info!("cancelled suspend");
        }
        Ok(())
    }
}

fn service_main_with_result(_args: Vec<OsString>) -> Result<()> {
    log::info!("started service");
    events::handle_events(AutoSleeper::default())
}

fn suspend() -> Result<()> {
    let ok = unsafe { windows::Win32::System::Power::SetSuspendState(false, true, true) };
    ok.ok().context("failed to SetSuspendState")
}
