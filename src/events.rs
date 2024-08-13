use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{select_biased, tick, unbounded};
use windows_service::{service::{PowerEventParam, ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, SessionChangeParam}, service_control_handler::{register, ServiceControlHandlerResult, ServiceStatusHandle}};

use crate::constants::{SERVICE_NAME, SERVICE_TYPE};

pub trait Callbacks {
    fn tick_duration(&self) -> Duration;

    // Tick.
    fn on_tick(&mut self, _now: Instant) -> Result<()> {
        Ok(())
    }
    // Events.
    fn listen(&self) -> ServiceControlAccept {
        ServiceControlAccept::POWER_EVENT | ServiceControlAccept::SESSION_CHANGE
    }
    fn on_power_event(&mut self, _power_event: PowerEventParam) -> Result<()> {
        Ok(())
    }
    fn on_session_change(&mut self, _session_change: SessionChangeParam) -> Result<()> {
        Ok(())
    }
}

fn on_event(callbacks: &mut impl Callbacks, event: ServiceControl) -> Result<()> {
    match event {
        ServiceControl::PowerEvent(power_event) => {
            callbacks.on_power_event(power_event)
        }
        ServiceControl::SessionChange(session_change) => {
            callbacks.on_session_change(session_change)
        }
        _ => {
            log::error!("unexpected event {event:?}");
            Ok(())
        }
    }
}

pub fn handle_events(mut callbacks: impl Callbacks) -> Result<()> {
    log::info!("preparing to listen to events");
    let events = callbacks.listen() | ServiceControlAccept::STOP;

    log::info!("preparing internal event handler");
    let (shutdown_tx, shutdown_rx) = unbounded();
    let (event_tx, event_rx) = unbounded();
    let event_handler = move |event: ServiceControl| -> ServiceControlHandlerResult {
        match event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                shutdown_tx.send(()).expect("shutdown_tx.send");
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::SessionChange(_) | ServiceControl::PowerEvent(_) => {
                event_tx.send(event).expect("event_tx.send");
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = register(SERVICE_NAME, event_handler)
        .with_context(|| format!("failed to register event handler for {SERVICE_NAME}"))?;
    log::info!("registered event handler");

    set_state(&status_handle, ServiceState::Running, events)?;

    let ticker = tick(callbacks.tick_duration());
    log::info!("ticking every {:?}", callbacks.tick_duration());
    loop {
        select_biased! {
            // Always try to shut down ASAP.
            recv(shutdown_rx) -> msg => {
                msg.context("failed to wait for shutdown")?;
                log::warn!("received shutdown request");
                set_state(&status_handle, ServiceState::Stopped, ServiceControlAccept::empty())?;
                return Ok(());
            }
            // Then handle the events. We want to notify the user ASAP.
            recv(event_rx) -> msg => {
                let event = msg.context("failed to receive event")?;
                on_event(&mut callbacks, event)?;
            }
            // And handle ticks last. This way the user should have received all
            // the information.
            recv(ticker) -> msg => {
                let now = msg.context("failed to receive tick")?;
                callbacks.on_tick(now)?;
            }
        }
    }
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
