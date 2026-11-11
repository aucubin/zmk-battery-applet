//! Tray icon, title and menu.

use anyhow::Result;
use tray_icon::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::model::{self, AppState, CentralSide};

pub const REFRESH_ID: &str = "refresh";
pub const QUIT_ID: &str = "quit";

pub struct Tray {
    icon: TrayIcon,
    central: CentralSide,
}

impl Tray {
    pub fn new(central: CentralSide) -> Result<Self> {
        let state = AppState::Starting;
        let builder = TrayIconBuilder::new();
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(keyboard_icon()?);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(keyboard_icon()?);
        let icon = builder
            .with_menu(Box::new(build_menu(&state, central)?))
            .with_title(model::title(&state, central))
            .with_tooltip(model::tooltip(&state, central))
            .build()?;
        Ok(Self { icon, central })
    }

    pub fn update(&self, state: &AppState) -> Result<()> {
        self.icon.set_title(Some(model::title(state, self.central)));
        self.icon
            .set_tooltip(Some(model::tooltip(state, self.central)))?;
        self.icon
            .set_menu(Some(Box::new(build_menu(state, self.central)?)));
        Ok(())
    }
}

fn build_menu(state: &AppState, central: CentralSide) -> Result<Menu> {
    let menu = Menu::new();
    for line in model::menu_lines(state, central) {
        menu.append(&MenuItem::new(line, false, None))?;
    }
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(
        MenuId::new(REFRESH_ID),
        "Refresh now",
        true,
        None,
    ))?;
    menu.append(&MenuItem::with_id(MenuId::new(QUIT_ID), "Quit", true, None))?;
    Ok(menu)
}

/// A small keyboard outline with a row of keys, drawn at runtime so no image assets are needed.
/// It is black for use as a macOS template image; Linux panels get a light grey.
fn keyboard_icon() -> Result<Icon> {
    const SIZE: u32 = 32;
    let color: [u8; 3] = if cfg!(target_os = "macos") {
        [0, 0, 0]
    } else {
        [0xdd; 3]
    };
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let mut set = |x: u32, y: u32| {
        let i = ((y * SIZE + x) * 4) as usize;
        rgba[i..i + 3].copy_from_slice(&color);
        rgba[i + 3] = 0xff;
    };
    // Outline: x 2..=29, y 8..=23, 2px thick.
    for x in 2..=29 {
        for y in [8, 9, 22, 23] {
            set(x, y);
        }
    }
    for y in 8..=23 {
        for x in [2, 3, 28, 29] {
            set(x, y);
        }
    }
    // Two rows of 2x2 keys and a space bar.
    for row in [12, 16] {
        for col in (6..=24).step_by(4) {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                set(col + dx, row + dy);
            }
        }
    }
    for x in 10..=21 {
        set(x, 19);
    }
    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}
