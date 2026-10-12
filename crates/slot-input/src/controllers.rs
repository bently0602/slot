use crate::Btn;

pub const STICK_DEADZONES: [u8; 6] = [10, 15, 20, 25, 30, 40];

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ControllerInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ControllerStatus {
    pub connected: Vec<ControllerInfo>,
    pub active: Option<String>,
    /// The player each connected controller plays as, from 1, in the order of `connected`.
    pub players: Vec<u8>,
    /// The player the handheld's own buttons play as, from 1. Zero means player one.
    pub handheld: u8,
}

impl ControllerStatus {
    pub fn handheld_player(&self) -> u8 {
        self.handheld.clamp(1, PLAYERS as u8)
    }
}

pub const PLAYERS: usize = 4;

/// What the external controllers are doing, which decides where their buttons go.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ControllerMode {
    /// Every controller drives the menus.
    #[default]
    Menu,
    /// Each controller plays its own player.
    Game,
    /// Each controller's left and right choose its player.
    Assign,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerProfile {
    pub id: String,
    pub name: String,
    pub swap_ab: bool,
    pub swap_xy: bool,
    pub deadzone: u8,
}

impl Default for ControllerProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            swap_ab: false,
            swap_xy: false,
            deadzone: 25,
        }
    }
}

impl ControllerProfile {
    pub fn map(&self, button: Btn) -> Btn {
        match button {
            Btn::A if self.swap_ab => Btn::B,
            Btn::B if self.swap_ab => Btn::A,
            Btn::X if self.swap_xy => Btn::Y,
            Btn::Y if self.swap_xy => Btn::X,
            _ => button,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerConfig {
    pub preferred: Option<String>,
    pub pause_disconnect: bool,
    pub profiles: Vec<ControllerProfile>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            preferred: None,
            pause_disconnect: true,
            profiles: Vec::new(),
        }
    }
}

impl ControllerConfig {
    pub fn profile(&self, id: &str) -> ControllerProfile {
        self.profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .unwrap_or_default()
    }
}
