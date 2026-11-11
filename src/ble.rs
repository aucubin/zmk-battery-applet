//! Finds connected ZMK split keyboards and reads their battery levels.
//!
//! ZMK exposes the central half's level as the first Battery Level characteristic of the
//! standard Battery Service. With `CONFIG_ZMK_SPLIT_BLE_CENTRAL_BATTERY_LEVEL_PROXY=y` every
//! peripheral half gets an extra Battery Level characteristic in the same service. A device with
//! at least two of them is treated as a split keyboard.

use std::collections::{HashMap, hash_map};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use bluest::btuuid::{characteristics, descriptors, services};
use bluest::{Adapter, Characteristic, Device, DeviceId};
use futures_util::StreamExt;
use futures_util::stream::select_all;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::model::{AppState, Half, Keyboard};

/// Fallback poll interval. It also catches disconnects and newly connected keyboards.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

pub type Publish = Arc<dyn Fn(AppState) + Send + Sync>;

struct Found {
    device: Device,
    keyboard: Keyboard,
    levels: Vec<Characteristic>,
}

struct Entry {
    id: DeviceId,
    keyboard: Keyboard,
}

type Entries = Arc<Mutex<Vec<Entry>>>;

/// Starts the monitor on a background thread. Notifying the returned handle forces a rescan.
pub fn spawn(publish: Publish) -> Arc<Notify> {
    let refresh = Arc::new(Notify::new());
    let refresh_bg = refresh.clone();
    std::thread::Builder::new()
        .name("ble".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");
            rt.block_on(run(publish, refresh_bg));
        })
        .expect("failed to spawn BLE thread");
    refresh
}

/// Scans once and returns the current state. Used by `--cli`.
pub async fn scan_once() -> Result<AppState> {
    let Some(adapter) = Adapter::default().await else {
        return Ok(AppState::BluetoothOff);
    };
    tokio::time::timeout(Duration::from_secs(10), adapter.wait_available())
        .await
        .context(
            "Bluetooth did not become available; is it on, and does this app/terminal \
             have Bluetooth permission (System Settings → Privacy & Security → Bluetooth)?",
        )??;
    let found = scan(&adapter).await?;
    Ok(AppState::Keyboards(
        found.into_iter().map(|f| f.keyboard).collect(),
    ))
}

async fn run(publish: Publish, refresh: Arc<Notify>) {
    let adapter = loop {
        match Adapter::default().await {
            Some(adapter) => break adapter,
            None => {
                log::warn!("no Bluetooth adapter found");
                publish(AppState::BluetoothOff);
                wait(&refresh).await;
            }
        }
    };

    let entries: Entries = Arc::default();
    let mut watchers: HashMap<DeviceId, JoinHandle<()>> = HashMap::new();

    loop {
        match scan(&adapter).await {
            Ok(found) => {
                // Stop watching devices that went away; (re)start watchers for the rest.
                watchers.retain(|id, handle| {
                    let keep = found.iter().any(|f| &f.device.id() == id) && !handle.is_finished();
                    if !keep {
                        handle.abort();
                    }
                    keep
                });
                {
                    let mut entries = entries.lock().unwrap();
                    *entries = found
                        .iter()
                        .map(|f| Entry {
                            id: f.device.id(),
                            keyboard: f.keyboard.clone(),
                        })
                        .collect();
                }
                for f in found {
                    let id = f.device.id();
                    if let hash_map::Entry::Vacant(slot) = watchers.entry(id.clone()) {
                        slot.insert(tokio::spawn(watch(
                            id,
                            f.levels,
                            entries.clone(),
                            publish.clone(),
                        )));
                    }
                }
                publish(snapshot(&entries));
            }
            Err(e) => {
                log::warn!("scan failed: {e:#}");
                for (_, handle) in watchers.drain() {
                    handle.abort();
                }
                entries.lock().unwrap().clear();
                publish(AppState::BluetoothOff);
                // Returns as soon as the adapter is powered on again.
                if let Err(e) = adapter.wait_available().await {
                    log::warn!("waiting for adapter failed: {e}");
                    wait(&refresh).await;
                }
                continue;
            }
        }
        wait(&refresh).await;
    }
}

async fn wait(refresh: &Notify) {
    tokio::select! {
        _ = tokio::time::sleep(POLL_INTERVAL) => {}
        _ = refresh.notified() => {}
    }
}

fn snapshot(entries: &Entries) -> AppState {
    AppState::Keyboards(
        entries
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.keyboard.clone())
            .collect(),
    )
}

async fn scan(adapter: &Adapter) -> Result<Vec<Found>> {
    let devices = adapter
        .connected_devices_with_services(&[services::BATTERY])
        .await
        .context("listing connected devices")?;
    let mut found = Vec::new();
    for device in devices {
        match read_keyboard(adapter, device).await {
            Ok(Some(f)) => found.push(f),
            Ok(None) => {}
            Err(e) => log::debug!("skipping device: {e:#}"),
        }
    }
    found.sort_by(|a, b| a.keyboard.name.cmp(&b.keyboard.name));
    Ok(found)
}

async fn read_keyboard(adapter: &Adapter, device: Device) -> Result<Option<Found>> {
    // Attaches to the existing OS connection; it does not create a second one.
    adapter.connect_device(&device).await.context("connect")?;

    let mut levels = Vec::new();
    for service in device
        .discover_services_with_uuid(services::BATTERY)
        .await?
    {
        levels.extend(
            service
                .discover_characteristics_with_uuid(characteristics::BATTERY_LEVEL)
                .await?,
        );
    }
    if levels.len() < 2 {
        // A regular single-battery device such as a mouse or a non-split keyboard.
        return Ok(None);
    }

    let mut halves = Vec::with_capacity(levels.len());
    for c in &levels {
        halves.push(Half {
            description: read_description(c).await,
            level: c.read().await.ok().and_then(|v| v.first().copied()),
        });
    }
    let name = device
        .name_async()
        .await
        .unwrap_or_else(|_| "ZMK keyboard".to_string());
    log::info!("found {name} with {} battery levels", halves.len());

    Ok(Some(Found {
        device,
        keyboard: Keyboard { name, halves },
        levels,
    }))
}

async fn read_description(c: &Characteristic) -> Option<String> {
    let descriptors = c.discover_descriptors().await.ok()?;
    let d = descriptors
        .iter()
        .find(|d| d.uuid() == descriptors::CHARACTERISTIC_USER_DESCRIPTION)?;
    let raw = d.read().await.ok()?;
    let text = String::from_utf8_lossy(&raw)
        .trim_end_matches('\0')
        .trim()
        .to_string();
    (!text.is_empty()).then_some(text)
}

/// Subscribes to level notifications so the tray updates without waiting for the next poll.
/// Ends when the device disconnects; the next scan then drops or re-adds it.
async fn watch(id: DeviceId, levels: Vec<Characteristic>, entries: Entries, publish: Publish) {
    let mut streams = Vec::new();
    for (index, c) in levels.iter().enumerate() {
        match c.notify().await {
            Ok(stream) => streams.push(stream.map(move |v| (index, v))),
            Err(e) => log::debug!("notify not available for level {index}: {e}"),
        }
    }
    if streams.is_empty() {
        // Nothing to watch; keep the task alive so the poll loop doesn't respawn it.
        std::future::pending::<()>().await;
    }

    let mut merged = select_all(streams);
    while let Some((index, value)) = merged.next().await {
        let Ok(value) = value else { break };
        let Some(&level) = value.first() else {
            continue;
        };
        let changed = {
            let mut entries = entries.lock().unwrap();
            match entries.iter_mut().find(|e| e.id == id) {
                Some(e) if e.keyboard.halves[index].level != Some(level) => {
                    e.keyboard.halves[index].level = Some(level);
                    true
                }
                _ => false,
            }
        };
        if changed {
            publish(snapshot(&entries));
        }
    }
}
