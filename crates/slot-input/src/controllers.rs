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
