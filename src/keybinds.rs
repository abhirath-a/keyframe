use std::collections::HashMap;

use egui::{InputState, Key, Modifiers};

use crate::config::Config;

/// Every action the presenter can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    NextPage,
    PrevPage,
    FirstPage,
    LastPage,
    ToggleHelp,
    ToggleFullscreen,
    Quit,
}

impl Action {
    /// Order controls how they're listed in the help screen.
    pub const ALL: [Action; 7] = [
        Action::NextPage,
        Action::PrevPage,
        Action::FirstPage,
        Action::LastPage,
        Action::ToggleHelp,
        Action::ToggleFullscreen,
        Action::Quit,
    ];

    /// Human-readable label for the help screen.
    pub fn label(self) -> &'static str {
        match self {
            Action::NextPage => "Next slide",
            Action::PrevPage => "Previous slide",
            Action::FirstPage => "First slide",
            Action::LastPage => "Last slide",
            Action::ToggleHelp => "Toggle this help screen",
            Action::ToggleFullscreen => "Toggle fullscreen",
            Action::Quit => "Quit",
        }
    }

    /// Stable snake_case name used as the key in config.toml's [keybinds].
    pub fn config_name(self) -> &'static str {
        match self {
            Action::NextPage => "next_page",
            Action::PrevPage => "prev_page",
            Action::FirstPage => "first_page",
            Action::LastPage => "last_page",
            Action::ToggleHelp => "toggle_help",
            Action::ToggleFullscreen => "toggle_fullscreen",
            Action::Quit => "quit",
        }
    }
}

/// A single key combination, e.g. "j" or "shift+G".
#[derive(Debug, Clone, Copy)]
pub struct KeyCombo {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyCombo {
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    pub fn shift(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::SHIFT,
        }
    }

    /// Parses strings like "Left", "space", "shift+g", "?", "F11".
    /// Returns `None` if the key name isn't recognized.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut modifiers = Modifiers::NONE;
        let mut rest = raw.trim();
        loop {
            let lower = rest.to_ascii_lowercase();
            if let Some(stripped) = lower.strip_prefix("shift+") {
                modifiers.shift = true;
                rest = &rest[rest.len() - stripped.len()..];
            } else if let Some(stripped) = lower.strip_prefix("ctrl+") {
                modifiers.ctrl = true;
                rest = &rest[rest.len() - stripped.len()..];
            } else if let Some(stripped) = lower.strip_prefix("alt+") {
                modifiers.alt = true;
                rest = &rest[rest.len() - stripped.len()..];
            } else if let Some(stripped) = lower.strip_prefix("cmd+") {
                modifiers.mac_cmd = true;
                modifiers.command = true;
                rest = &rest[rest.len() - stripped.len()..];
            } else {
                break;
            }
        }
        let key = Key::from_name(rest)?;
        Some(Self { key, modifiers })
    }

    /// True if this combo matches what's currently held/pressed this frame.
    fn matches(&self, input: &InputState) -> bool {
        input.key_pressed(self.key)
            && input.modifiers.shift == self.modifiers.shift
            && input.modifiers.ctrl == self.modifiers.ctrl
            && input.modifiers.alt == self.modifiers.alt
            && input.modifiers.mac_cmd == self.modifiers.mac_cmd
    }

    /// Short display string for the help screen, e.g. "Shift+G".
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.modifiers.ctrl {
            s.push_str("Ctrl+");
        }
        if self.modifiers.alt {
            s.push_str("Alt+");
        }
        if self.modifiers.mac_cmd {
            s.push_str("Cmd+");
        }
        if self.modifiers.shift {
            s.push_str("Shift+");
        }
        s.push_str(key_display_name(self.key));
        s
    }
}

fn key_display_name(key: Key) -> &'static str {
    match key {
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::Space => "Space",
        Key::Backspace => "Backspace",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "Page Up",
        Key::PageDown => "Page Down",
        Key::Escape => "Esc",
        Key::Questionmark => "?",
        Key::F => "F",
        Key::F11 => "F11",
        Key::G => "G",
        Key::H => "H",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::N => "N",
        Key::P => "P",
        Key::Q => "Q",
        Key::F1 => "F1",
        _ => "?",
    }
}

/// Default keybinds: both "normie" (arrows/space/home/end) and vim-style
/// bindings are active simultaneously out of the box.
pub fn default_keybinds() -> HashMap<Action, Vec<KeyCombo>> {
    use Key::*;
    let mut m: HashMap<Action, Vec<KeyCombo>> = HashMap::new();

    m.insert(
        Action::NextPage,
        vec![
            KeyCombo::plain(ArrowRight),
            KeyCombo::plain(ArrowDown),
            KeyCombo::plain(Space),
            KeyCombo::plain(PageDown),
            KeyCombo::plain(J), // vim: down
            KeyCombo::plain(L), // vim: right
            KeyCombo::plain(N), // vim: next
        ],
    );
    m.insert(
        Action::PrevPage,
        vec![
            KeyCombo::plain(ArrowLeft),
            KeyCombo::plain(ArrowUp),
            KeyCombo::plain(Backspace),
            KeyCombo::plain(PageUp),
            KeyCombo::plain(K), // vim: up
            KeyCombo::plain(H), // vim: left
            KeyCombo::plain(P), // vim: previous
        ],
    );
    m.insert(
        Action::FirstPage,
        vec![KeyCombo::plain(Home), KeyCombo::plain(G)],
    );
    m.insert(
        Action::LastPage,
        vec![KeyCombo::plain(End), KeyCombo::shift(G)],
    );
    m.insert(
        Action::ToggleHelp,
        vec![KeyCombo::plain(Questionmark), KeyCombo::plain(F1)],
    );
    m.insert(
        Action::ToggleFullscreen,
        vec![KeyCombo::plain(F), KeyCombo::plain(F11)],
    );
    m.insert(Action::Quit, vec![KeyCombo::plain(Q)]);

    m
}

/// Starts from `default_keybinds()`, then replaces any action that has an
/// override in `config.keybinds`.
pub fn resolve_keybinds(config: &Config) -> HashMap<Action, Vec<KeyCombo>> {
    let mut binds = default_keybinds();

    for action in Action::ALL {
        if let Some(raw) = config.keybinds.get(action.config_name()) {
            let parsed: Vec<KeyCombo> = raw
                .iter()
                .filter_map(|s| {
                    let combo = KeyCombo::parse(s);
                    if combo.is_none() {
                        eprintln!("warning: unknown key '{s}' for {}", action.config_name());
                    }
                    combo
                })
                .collect();
            if !parsed.is_empty() {
                binds.insert(action, parsed);
            }
        }
    }

    binds
}

/// Returns the first action whose keybind matches this frame's input, if any.
pub fn triggered_action(
    binds: &HashMap<Action, Vec<KeyCombo>>,
    input: &InputState,
) -> Option<Action> {
    for action in Action::ALL {
        if let Some(combos) = binds.get(&action) {
            if combos.iter().any(|c| c.matches(input)) {
                return Some(action);
            }
        }
    }
    None
}
