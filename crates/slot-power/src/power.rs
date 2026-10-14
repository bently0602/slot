use std::time::Duration;

use crate::{Battery, Charge, LedState, LidPolicy, Platform};

pub struct Power {
    platform: Box<dyn Platform>,
    level: u8,
    closed: bool,
    screen_off: bool,
    timeout: Duration,
}

impl Power {
    pub fn new(platform: Box<dyn Platform>, timeout: Duration) -> Self {
        Power {
            platform,
            level: 0,
            closed: false,
            screen_off: false,
            timeout,
        }
    }

    pub fn set_backlight(&mut self, step: u8) {
        self.level = step;
        if !self.closed && !self.screen_off {
            self.platform.set_backlight(step);
        }
    }

    pub fn set_screen_off(&mut self, off: bool) {
        self.screen_off = off;
        self.platform
            .set_backlight(if off || self.closed { 0 } else { self.level });
    }

    pub fn battery(&self) -> Option<Battery> {
        self.platform.battery()
    }

    /// Zero disables automatic shutdown during software standby.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn charge(&self) -> Charge {
        self.platform.charge()
    }

    pub fn headphones(&self) -> bool {
        self.platform.headphones()
    }

    pub fn set_led(&mut self, state: LedState) {
        self.platform.set_led(state)
    }

    pub fn set_rumble(&mut self, strength: u16) {
        self.platform.set_rumble(strength);
    }

    pub fn relink_adb(&mut self) -> bool {
        self.platform.relink_adb()
    }

    pub fn poweroff(&mut self) -> ! {
        self.platform.poweroff()
    }

    pub fn restart(&mut self) -> ! {
        self.platform.restart()
    }

    pub fn now(&self) -> i64 {
        self.platform.now()
    }

    pub fn set_clock(&mut self, secs: i64) {
        self.platform.set_clock(secs);
    }
}

impl LidPolicy for Power {
    fn on_close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.platform.set_backlight(0);
    }

    fn on_open(&mut self) {
        self.closed = false;
        self.platform
            .set_backlight(if self.screen_off { 0 } else { self.level });
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }
}
