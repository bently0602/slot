use std::time::{Duration, Instant};

use slot::frontend::Frontend;
use slot::input::HostInput;
use slot_gfx::{Compositor, HostSurface, Surface};
use slot_power::SimPlatform;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

const FRAME: Duration = Duration::from_micros(16_667);

struct Slot {
    gfx: Option<(HostSurface, Compositor)>,
    frontend: Frontend,
    input: HostInput,
    hdmi_preview: bool,
    device_info: Option<std::path::PathBuf>,
}

impl Slot {
    fn new() -> Self {
        let hdmi_preview = std::env::var("SLOT_HDMI_PREVIEW").is_ok_and(|v| v == "1");
        let mut frontend = Frontend::boot(Box::new(SimPlatform::new()));
        frontend.set_docked(hdmi_preview);
        Slot {
            gfx: None,
            frontend,
            input: HostInput::new(),
            hdmi_preview,
            device_info: None,
        }
    }
}

impl ApplicationHandler for Slot {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let built = HostSurface::new(events).and_then(|s| Compositor::new(&s).map(|c| (s, c)));
        let (surface, mut compositor) = match built {
            Ok(gfx) => gfx,
            Err(e) => {
                eprintln!("slot: {e}");
                events.exit();
                return;
            }
        };
        self.frontend.upload_faces(&mut compositor);
        self.frontend
            .upload_bezel(&mut compositor, surface.window_size());
        self.gfx = Some((surface, compositor));
    }

    fn window_event(&mut self, events: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.input.on_window_event(&event);
        let Some((surface, compositor)) = self.gfx.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => events.exit(),
            WindowEvent::Resized(size) => surface.resize(size),
            WindowEvent::RedrawRequested => {
                let began = Instant::now();
                compositor
                    .set_hdmi_scaling(self.hdmi_preview.then(|| self.frontend.hdmi_scaling()));
                self.frontend.render(compositor, surface.window_size());
                let swap = std::time::Instant::now();
                let swapped = surface.swap();
                slot::latency::swapped(swap.elapsed().as_secs_f64() * 1000.0);
                if let Err(e) = swapped {
                    eprintln!("slot: {e}");
                    events.exit();
                    return;
                }
                surface.request_redraw();
                self.frontend.advance(&mut self.input);
                if let Some(root) = self.frontend.take_device_info_request() {
                    self.device_info = Some(root);
                    events.exit();
                    return;
                }
                if self.frontend.restarting() {
                    self.frontend.restart();
                }
                if self.frontend.powering_off() {
                    self.frontend.poweroff();
                    events.exit();
                    return;
                }
                if let Some(left) = FRAME.checked_sub(began.elapsed()) {
                    std::thread::sleep(left);
                }
            }
            _ => {}
        }
    }
}

pub fn run() {
    let events = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("slot: {e}");
            return;
        }
    };
    events.set_control_flow(ControlFlow::Poll);
    let mut app = Slot::new();
    if let Err(e) = events.run_app(&mut app) {
        eprintln!("slot: {e}");
    }
    let device_info = app.device_info.take();
    drop(app);
    if let Some(root) = device_info {
        slot::system_app::device_info(&root);
    }
}
