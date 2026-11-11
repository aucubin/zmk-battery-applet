# zmk-battery-applet

A small menu bar (macOS) / system tray (Linux) applet that shows the battery level of **both halves** of a
[ZMK](https://zmk.dev) split keyboard connected over Bluetooth.

```
⌨ L 85% · R 72%
```

<!-- TODO: screenshot -->

- Detects every connected ZMK split keyboard automatically. With more than one, the label is compact (`85/72 · 90/88`)
  and the menu lists each keyboard.
- Updates live through BLE notifications, and also polls every 30 s.
- Shows `—` when no keyboard is connected and `BT off` when Bluetooth is off. It picks the keyboard up again on its own
  after it reconnects.
- "Refresh now" in the menu rescans immediately.

## Keyboard firmware requirement

By default ZMK only reports the central half's battery. To make the peripheral half visible too, add this to your
keyboard's `.conf` and reflash the **central** half:

```
CONFIG_ZMK_SPLIT_BLE_CENTRAL_BATTERY_LEVEL_FETCHING=y
CONFIG_ZMK_SPLIT_BLE_CENTRAL_BATTERY_LEVEL_PROXY=y
```

The applet treats any connected device whose Battery Service has two or more Battery Level characteristics as a split
keyboard. The first characteristic is the central half. The applet assumes the central half is on the left; if yours
is on the right, start it with `--central-right`.

## Download

Every push to `main` builds the app in GitHub Actions. Open the latest
[build run](../../actions/workflows/build.yml) and download the artifact for your platform:

- `zmk-battery-applet-macos-universal.zip`: `ZMK Battery.app` (Apple Silicon and Intel)
- `zmk-battery-applet-linux-x86_64`: the binary. Make it executable after downloading
  (`chmod +x zmk-battery-applet-linux-x86_64`)

The macOS app is only ad-hoc signed. The first time, right-click → Open (or run
`xattr -dr com.apple.quarantine "ZMK Battery.app"`).

## Building

Requires Rust 1.99 (pinned in `rust-toolchain.toml`).

### macOS

```sh
cargo build --release
scripts/bundle-macos.sh            # → target/bundle/ZMK Battery.app
open "target/bundle/ZMK Battery.app"
```

Run the app as a bundle: macOS only asks for Bluetooth permission for apps with an `Info.plist`. Accept the prompt,
or enable it later under **System Settings → Privacy & Security → Bluetooth**. To start it at login, add the app
under **System Settings → General → Login Items**.

### Linux

```sh
# Debian/Ubuntu
sudo apt install libgtk-3-dev libayatana-appindicator3-dev libdbus-1-dev libxdo-dev pkg-config
# Fedora
sudo dnf install gtk3-devel libayatana-appindicator-gtk3-devel dbus-devel libxdo-devel pkgconf

cargo build --release
./target/release/zmk-battery-applet
```

You need a tray that supports StatusNotifierItem/AppIndicator. KDE and most other desktops have one. On GNOME, install
the *AppIndicator and KStatusNotifierItem Support* extension. The text label shows up where the tray supports
AppIndicator labels.

#### BlueZ battery plugin

BlueZ's built-in `battery` plugin may claim the Battery Service and hide it from other applications, so only one level
is visible (e.g. in upower). If the applet doesn't find your keyboard, check with:

```sh
busctl tree org.bluez | grep -i service   # look for the 180f service under your keyboard
```

If the service is missing, disable the plugin:

```sh
sudo systemctl edit bluetooth
# [Service]
# ExecStart=
# ExecStart=/usr/libexec/bluetooth/bluetoothd -P battery
sudo systemctl restart bluetooth
```

(the path to `bluetoothd` varies by distribution; see `systemctl cat bluetooth`).

## Usage

```
zmk-battery-applet [--cli] [--central-right]

  --cli            Print the battery levels once and exit
  --central-right  The central half is the right one (default: left)
```

Use `RUST_LOG=debug` for verbose logs, which include the Bluetooth authorization state on macOS.

## License

[MIT](LICENSE)
