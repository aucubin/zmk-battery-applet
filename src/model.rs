//! Plain data describing what the applet shows, plus the text formatting for it.

/// Levels below this are flagged in the menu.
pub const LOW_BATTERY: u8 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    /// No scan has completed yet.
    Starting,
    /// The Bluetooth adapter is missing or switched off.
    BluetoothOff,
    /// Result of the last scan. Empty means no ZMK split keyboard is connected.
    Keyboards(Vec<Keyboard>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyboard {
    pub name: String,
    /// The central half comes first, followed by each peripheral.
    pub halves: Vec<Half>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Half {
    /// Name from the characteristic's User Description descriptor, if any.
    pub description: Option<String>,
    pub level: Option<u8>,
}

/// Which physical side the central half is on. ZMK splits usually use the left half as central.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CentralSide {
    #[default]
    Left,
    Right,
}

impl Keyboard {
    /// Short label for half `index`: L/R for a regular two-part split, a number otherwise.
    pub fn short_label(&self, index: usize, central: CentralSide) -> String {
        if self.halves.len() == 2 {
            let central_first = central == CentralSide::Left;
            return if (index == 0) == central_first {
                "L"
            } else {
                "R"
            }
            .to_string();
        }
        (index + 1).to_string()
    }

    /// Long label for the menu, e.g. "Left (central)".
    pub fn long_label(&self, index: usize, central: CentralSide) -> String {
        let role = if index == 0 { "central" } else { "peripheral" };
        let side = if self.halves.len() == 2 {
            match self.short_label(index, central).as_str() {
                "L" => "Left".to_string(),
                _ => "Right".to_string(),
            }
        } else {
            match &self.halves[index].description {
                Some(d) if index > 0 => d.clone(),
                _ => format!("Part {}", index + 1),
            }
        };
        format!("{side} ({role})")
    }
}

fn level_text(level: Option<u8>) -> String {
    match level {
        Some(l) => format!("{l}%"),
        None => "?".to_string(),
    }
}

/// Text shown next to the tray icon.
pub fn title(state: &AppState, central: CentralSide) -> String {
    match state {
        AppState::Starting => "…".to_string(),
        AppState::BluetoothOff => "BT off".to_string(),
        AppState::Keyboards(kbs) if kbs.is_empty() => "—".to_string(),
        AppState::Keyboards(kbs) if kbs.len() == 1 => {
            let kb = &kbs[0];
            (0..kb.halves.len())
                .map(|i| {
                    format!(
                        "{} {}",
                        kb.short_label(i, central),
                        level_text(kb.halves[i].level)
                    )
                })
                .collect::<Vec<_>>()
                .join(" · ")
        }
        AppState::Keyboards(kbs) => kbs
            .iter()
            .map(|kb| {
                kb.halves
                    .iter()
                    .map(|h| h.level.map_or("?".to_string(), |l| l.to_string()))
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect::<Vec<_>>()
            .join(" · "),
    }
}

/// Detail lines for the menu: one header per keyboard followed by its halves.
pub fn menu_lines(state: &AppState, central: CentralSide) -> Vec<String> {
    match state {
        AppState::Starting => vec!["Looking for keyboards…".to_string()],
        AppState::BluetoothOff => vec!["Bluetooth is off or not permitted".to_string()],
        AppState::Keyboards(kbs) if kbs.is_empty() => {
            vec!["No ZMK keyboard connected".to_string()]
        }
        AppState::Keyboards(kbs) => {
            let mut lines = Vec::new();
            for kb in kbs {
                lines.push(kb.name.clone());
                for (i, half) in kb.halves.iter().enumerate() {
                    let warn = match half.level {
                        Some(l) if l < LOW_BATTERY => " ⚠",
                        _ => "",
                    };
                    lines.push(format!(
                        "    {}: {}{warn}",
                        kb.long_label(i, central),
                        level_text(half.level)
                    ));
                }
            }
            lines
        }
    }
}

/// Multi-line tooltip text.
pub fn tooltip(state: &AppState, central: CentralSide) -> String {
    let mut lines = vec!["ZMK Battery".to_string()];
    lines.extend(
        menu_lines(state, central)
            .into_iter()
            .map(|l| l.trim().to_string()),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(name: &str, levels: &[Option<u8>]) -> Keyboard {
        Keyboard {
            name: name.to_string(),
            halves: levels
                .iter()
                .map(|&level| Half {
                    description: None,
                    level,
                })
                .collect(),
        }
    }

    #[test]
    fn title_single_keyboard() {
        let state = AppState::Keyboards(vec![kb("Corne", &[Some(85), Some(72)])]);
        assert_eq!(title(&state, CentralSide::Left), "L 85% · R 72%");
        assert_eq!(title(&state, CentralSide::Right), "R 85% · L 72%");
    }

    #[test]
    fn title_two_keyboards() {
        let state = AppState::Keyboards(vec![
            kb("Corne", &[Some(85), Some(72)]),
            kb("Lily", &[Some(90), Some(88)]),
        ]);
        assert_eq!(title(&state, CentralSide::Left), "85/72 · 90/88");
    }

    #[test]
    fn title_no_keyboard_and_bt_off() {
        assert_eq!(title(&AppState::Keyboards(vec![]), CentralSide::Left), "—");
        assert_eq!(title(&AppState::BluetoothOff, CentralSide::Left), "BT off");
    }

    #[test]
    fn title_missing_level() {
        let state = AppState::Keyboards(vec![kb("Corne", &[Some(85), None])]);
        assert_eq!(title(&state, CentralSide::Left), "L 85% · R ?");
    }

    #[test]
    fn three_part_split_uses_numbers() {
        let mut k = kb("Big", &[Some(1), Some(2), Some(3)]);
        k.halves[2].description = Some("Thumb".to_string());
        let state = AppState::Keyboards(vec![k]);
        assert_eq!(title(&state, CentralSide::Left), "1 1% · 2 2% · 3 3%");
        let lines = menu_lines(&state, CentralSide::Left);
        assert_eq!(lines[1], "    Part 1 (central): 1% ⚠");
        assert_eq!(lines[3], "    Thumb (peripheral): 3% ⚠");
    }

    #[test]
    fn menu_lines_for_split() {
        let state = AppState::Keyboards(vec![kb("Corne", &[Some(85), Some(10)])]);
        assert_eq!(
            menu_lines(&state, CentralSide::Left),
            vec![
                "Corne".to_string(),
                "    Left (central): 85%".to_string(),
                "    Right (peripheral): 10% ⚠".to_string(),
            ]
        );
        assert_eq!(
            menu_lines(&AppState::Keyboards(vec![]), CentralSide::Left),
            vec!["No ZMK keyboard connected".to_string()]
        );
    }
}
