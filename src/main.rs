mod ble;
mod model;
mod tray;

use std::sync::{Arc, Mutex};

use anyhow::Result;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::MenuEvent;

use crate::model::{AppState, CentralSide};

enum UserEvent {
    State(AppState),
    Menu(MenuEvent),
}

const USAGE: &str = "\
Usage: zmk-battery-applet [--cli] [--central-right]

  --cli            Print the battery levels once and exit
  --central-right  The central half is the right one (default: left)";

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut cli = false;
    let mut central = CentralSide::Left;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--cli" => cli = true,
            "--central-right" => central = CentralSide::Right,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => anyhow::bail!("unknown argument: {other}\n\n{USAGE}"),
        }
    }

    if cli {
        let state = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(ble::scan_once())?;
        for line in model::menu_lines(&state, central) {
            println!("{line}");
        }
        return Ok(());
    }

    run_tray(central)
}

fn run_tray(central: CentralSide) -> Result<()> {
    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    // EventLoopProxy is not Sync, so guard it for use from the BLE and menu callbacks.
    let proxy = Arc::new(Mutex::new(event_loop.create_proxy()));
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy
            .lock()
            .unwrap()
            .send_event(UserEvent::Menu(event));
    }));

    let mut tray = None;
    let mut refresh = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // The tray must be created once the event loop is running (required on macOS).
            Event::NewEvents(StartCause::Init) => {
                match tray::Tray::new(central) {
                    Ok(t) => tray = Some(t),
                    Err(e) => {
                        log::error!("failed to create tray icon: {e:#}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
                let proxy = proxy.clone();
                refresh = Some(ble::spawn(Arc::new(move |state| {
                    let _ = proxy.lock().unwrap().send_event(UserEvent::State(state));
                })));
            }
            Event::UserEvent(UserEvent::State(state)) => {
                if let Some(t) = &tray
                    && let Err(e) = t.update(&state)
                {
                    log::warn!("failed to update tray: {e:#}");
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) => match event.id.as_ref() {
                tray::REFRESH_ID => {
                    if let Some(r) = &refresh {
                        r.notify_one();
                    }
                }
                tray::QUIT_ID => {
                    tray.take();
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            _ => {}
        }
    });
}
