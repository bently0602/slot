#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Btn {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Start,
    Select,
    Menu,
    VolUp,
    VolDown,
    Power,
    Lid,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RawEvent {
    Down(Btn),
    Up(Btn),
}

pub type Millis = u64;

pub trait InputSource {
    fn poll(&mut self, now: Millis) -> Vec<RawEvent>;
    fn configure_controllers(&mut self, _: &crate::ControllerConfig) {}
    fn controller_status(&self) -> crate::ControllerStatus {
        crate::ControllerStatus::default()
    }
    fn set_controller_mode(&mut self, _: crate::ControllerMode) {}
    /// The buttons each player holds on a controller that is theirs alone. A controller
    /// that shares the handheld's player is part of `poll`'s stream instead.
    fn player_buttons(&self) -> [Vec<Btn>; crate::PLAYERS] {
        Default::default()
    }
}
