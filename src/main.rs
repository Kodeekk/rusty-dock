use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
        client::{
            protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface, wl_region},
            Connection, QueueHandle,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::globals::registry_queue_init;
use std::time::Instant;
use device_query::{DeviceQuery, DeviceState};

mod app_launcher;
mod config;
mod renderer;
mod config_window_iced;
use crate::config_window_iced as config_window;
mod animations;
mod bootstrap;
mod process_monitor;

use app_launcher::AppLauncher;
use config::DockConfig;
use renderer::{DockRenderer, OVERDRAW};
use animations::DockAnimations;
use process_monitor::ProcessMonitor;
use crate::bootstrap::imagemagick_probe;

// ── Surface constants ─────────────────────────────────────────────────────────
//   Surface layout (y=0 at top):
//   [0 .. POPUP_RESERVE)            → folder popup zone
//   [POPUP_RESERVE .. OVERDRAW)     → bounce animation overdraw zone
//   [OVERDRAW .. total_height)      → visible dock panel

// ── DockApp ───────────────────────────────────────────────────────────────────

struct DockApp {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm_state: Shm,
    layer_shell: LayerShell,
    loop_handle: LoopHandle<'static, Self>,

    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<LayerSurface>,
    pointer: Option<wl_pointer::WlPointer>,
    pool: Option<SlotPool>,

    width: u32,
    height: u32,   // = config.dock_height + OVERDRAW

    renderer: DockRenderer,
    launcher: AppLauncher,
    animations: DockAnimations,
    config: DockConfig,
    proc_monitor: ProcessMonitor,

    // Mouse state
    mouse_x: f64,
    mouse_y: f64,
    hovered_app: Option<usize>,

    // Drag & drop
    dragging_app: Option<usize>,
    drag_start_x: f64,
    drag_offset_x: f64,

    // Folder state
    open_folder: Option<usize>,
    hovered_folder_app: Option<usize>,

    // ── Auto-hide / smart-hide ────────────────────────────────────────────────
    is_mouse_inside: bool,
    mouse_leave_time: Option<Instant>,
    smart_trigger_start: Option<Instant>,
    smart_last_y: f64,
    dock_revealed: bool,
    device_state: DeviceState,

    configured: bool,
    exit: bool,
    last_reload_check: Instant,

    needs_redraw: bool,
    frame_pending: bool,

    qh: Option<QueueHandle<Self>>,
}

impl DockApp {
    fn new(
        registry_state: RegistryState,
        seat_state: SeatState,
        output_state: OutputState,
        compositor_state: CompositorState,
        shm_state: Shm,
        layer_shell: LayerShell,
        loop_handle: LoopHandle<'static, Self>,
    ) -> Self {
        let config    = DockConfig::load().unwrap_or_default();
        let launcher  = AppLauncher::new(config.pinned_apps.clone(), config.debug);
        let renderer  = DockRenderer::new();
        let animations = DockAnimations::new();
        let proc_monitor = ProcessMonitor::new();
        let height = config.dock_height + OVERDRAW;

        Self {
            registry_state, seat_state, output_state, compositor_state,
            shm_state, layer_shell, loop_handle,
            surface: None, layer_surface: None, pointer: None, pool: None,
            width: 1920, height,
            renderer, launcher, animations, config, proc_monitor,
            mouse_x: 0.0, mouse_y: 0.0,
            hovered_app: None,
            dragging_app: None, drag_start_x: 0.0, drag_offset_x: 0.0,
            open_folder: None, hovered_folder_app: None,
            is_mouse_inside: false,
            mouse_leave_time: None,
            smart_trigger_start: None,
            smart_last_y: -1.0,
            dock_revealed: false,
            device_state: DeviceState::new(),
            configured: false, exit: false,
            last_reload_check: Instant::now(),
            needs_redraw: false,
            frame_pending: false,
            qh: None,
        }
    }

    fn check_reload_signal(&mut self) {
        if self.last_reload_check.elapsed().as_millis() < 500 { return; }
        self.last_reload_check = Instant::now();
        if let Ok(path) = config_window::reload_signal_path() {
            if path.exists() {
                let _ = std::fs::remove_file(&path);
                self.reload_config();
            }
        }
    }

    fn reload_config(&mut self) {
        if let Ok(cfg) = DockConfig::load() {
            self.config = cfg.clone();
            self.launcher = AppLauncher::new(cfg.pinned_apps.clone(), cfg.debug);
            self.height = cfg.dock_height + OVERDRAW;
            self.open_folder = None;
            self.resize_dock();
        }
    }

    fn poll_processes(&mut self) {
        if !self.proc_monitor.should_poll() { return; }
        for idx in self.proc_monitor.poll_dead() {
            if idx >= 10_000 {
                let folder_idx = idx / 10_000;
                let app_idx = idx % 10_000;
                if let Some(folder) = self.launcher.entries.get_mut(folder_idx) {
                    if let Some(fe) = folder.folder_entries.get_mut(app_idx) {
                        if fe.is_active {
                            fe.is_active = false;
                            self.needs_redraw = true;
                        }
                    }
                }
            } else if let Some(e) = self.launcher.entries.get_mut(idx) {
                if e.is_active { e.is_active = false; self.needs_redraw = true; }
            }
        }
    }

    fn init_surface(&mut self, qh: &QueueHandle<Self>) {
        self.qh = Some(qh.clone());
        let surface = self.compositor_state.create_surface(qh);
        let layer_surface = self.layer_shell.create_layer_surface(
            qh, surface.clone(), Layer::Top, Some("rusty-dock"), None,
        );
        let dock_w = self.calculate_dock_width();
        layer_surface.set_anchor(Anchor::BOTTOM);
        layer_surface.set_size(dock_w, self.height);
        layer_surface.set_exclusive_zone(0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.set_margin(-(OVERDRAW as i32), 0, self.config.dock_margin, 0);
        layer_surface.commit();
        self.surface = Some(surface);
        self.layer_surface = Some(layer_surface);
        self.width = dock_w;
    }

    fn calculate_dock_width(&self) -> u32 {
        let n = self.launcher.entries.len() as u32;
        if n == 0 { return 200; }
        n * (self.config.icon_size + self.config.icon_padding * 2) + 40
    }

    fn resize_dock(&mut self) {
        let new_w = self.calculate_dock_width();
        let new_h = self.config.dock_height + OVERDRAW;
        if new_w != self.width || new_h != self.height {
            self.width = new_w; self.height = new_h;
            if let Some(ls) = &self.layer_surface {
                ls.set_size(new_w, new_h);
                ls.set_margin(-(OVERDRAW as i32), 0, self.config.dock_margin, 0);
                ls.commit();
            }
            if let Some(pool) = &mut self.pool {
                pool.resize((self.width * self.height * 4) as usize).expect("resize pool");
            }
            self.needs_redraw = true;
        }
    }

    fn update_animations(&mut self) {
        if self.config.auto_hide {
            let visible = self.is_mouse_inside && self.dock_revealed;
            if self.config.enable_animations {
                self.animations.start_visibility_animation(
                    visible, self.config.animation_duration_ms,
                );
            }
        }

        if self.config.enable_animations {
            for i in 0..self.launcher.entries.len() {
                let target = if self.hovered_app == Some(i) && self.dragging_app.is_none()
                    && self.open_folder.is_none()
                { self.config.icon_zoom_on_hover } else { 1.0 };
                self.animations.start_icon_scale(i, target, self.config.animation_duration_ms);
            }
        }

        if self.animations.is_animating() { self.needs_redraw = true; }
    }

    fn tick_smart_hide(&mut self) {
        if !self.config.auto_hide || !self.config.smart_hide { return; }
        if self.dock_revealed { return; }

        let mouse = self.device_state.get_mouse();
        let (_, my) = mouse.coords;

        let monitor_height = self.output_state.outputs().next()
            .and_then(|o| self.output_state.info(&o))
            .and_then(|info| info.logical_size)
            .map(|(_, h)| h)
            .unwrap_or(1080) as i32;

        if my >= monitor_height - 10 {
            if self.smart_trigger_start.is_none() {
                self.smart_trigger_start = Some(Instant::now());
            } else if self.smart_trigger_start.unwrap().elapsed().as_millis() >= self.config.smart_hide_trigger_delay_ms as u128 {
                self.dock_revealed = true;
                self.is_mouse_inside = true;
                self.mouse_leave_time = None;
                self.needs_redraw = true;
                if self.config.enable_animations {
                    self.animations.start_visibility_animation(true, self.config.animation_duration_ms);
                }
            }
        } else {
            self.smart_trigger_start = None;
        }
    }

    fn update_input_region(&mut self) {
        let qh = match &self.qh { Some(q) => q, None => return };
        let surface = match &self.surface { Some(s) => s, None => return };

        let region = self.compositor_state.wl_compositor().create_region(qh, ());
        let visibility = if self.config.auto_hide {
            self.animations.get_visibility()
        } else { 1.0 };

        if visibility > 0.01 {
            let panel_y = OVERDRAW as i32;
            let panel_h = self.config.dock_height as i32;
            region.add(0, panel_y, self.width as i32, panel_h);

            if let Some(folder_idx) = self.open_folder {
                if let Some((px, py, pw, ph)) = self.renderer.popup_rect(
                    folder_idx, &self.launcher.entries, self.width, &self.config
                ) {
                    region.add(px as i32, py as i32, pw as i32, ph as i32);

                    let icon_size = self.config.icon_size as i32;
                    let icon_padding = self.config.icon_padding as i32;
                    let n = self.launcher.entries.len() as i32;
                    let total_w = n * (icon_size + icon_padding * 2);
                    let start_x = (self.width as i32 - total_w) / 2;
                    let folder_x = start_x + (folder_idx as i32 * (icon_size + icon_padding * 2));

                    let bridge_w = icon_size + icon_padding * 2;
                    let bridge_x = folder_x;
                    let bridge_y = (py + ph) as i32;
                    let bridge_h = panel_y - bridge_y;

                    if bridge_h > 0 {
                        region.add(bridge_x, bridge_y, bridge_w, bridge_h);
                    }
                }
            }
            surface.set_input_region(Some(&region));
        } else {
            surface.set_input_region(Some(&region));
        }
        region.destroy();
    }

    fn draw(&mut self) {
        if !self.configured { return; }
        let surface = match &self.surface { Some(s) => s.clone(), None => return };

        let pool = self.pool.get_or_insert_with(|| {
            SlotPool::new(self.width as usize * self.height as usize * 4, &self.shm_state)
                .expect("Failed to create SlotPool")
        });

        let result = pool.create_buffer(
            self.width as i32, self.height as i32,
            self.width as i32 * 4, wl_shm::Format::Argb8888,
        );
        let (buffer, canvas) = match result {
            Ok(r) => r,
            Err(e) => { eprintln!("Buffer alloc failed (will retry): {:?}", e); return; }
        };

        let visibility = if self.config.auto_hide {
            self.animations.get_visibility()
        } else { 1.0 };

        self.renderer.render(
            canvas, self.width, self.height,
            &self.launcher.entries,
            self.hovered_app, self.dragging_app, self.drag_offset_x,
            visibility, &mut self.animations, &self.config,
            self.open_folder, self.hovered_folder_app,
        );

        self.update_input_region();

        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        if let Some(qh) = &self.qh {
            surface.frame(qh, surface.clone());
            self.frame_pending = true;
        }
        surface.commit();
    }

    fn handle_press(&mut self, x: f64, _y: f64) {
        if self.animations.get_visibility() < 0.5 { return; }
        if let Some(app_idx) = self.hovered_app {
            if app_idx < self.launcher.entries.len() {
                self.dragging_app = Some(app_idx);
                self.drag_start_x = x;
                self.drag_offset_x = 0.0;
            }
        }
    }

    fn handle_release(&mut self, _x: f64, _y: f64) {
        if let (Some(folder_idx), Some(app_idx)) = (self.open_folder, self.hovered_folder_app) {
            if let Some(pid) = self.launcher.launch_folder_app(folder_idx, app_idx) {
                self.proc_monitor.register(folder_idx * 10_000 + app_idx, pid);
            }
            self.open_folder = None;
            self.hovered_folder_app = None;
            self.needs_redraw = true;
            return;
        }

        if let Some(dragging_idx) = self.dragging_app {
            if self.drag_offset_x.abs() < 5.0 {
                let entry = &self.launcher.entries[dragging_idx];

                if entry.is_folder {
                    self.open_folder = if self.open_folder == Some(dragging_idx) {
                        None
                    } else {
                        Some(dragging_idx)
                    };
                    self.hovered_folder_app = None;
                    self.needs_redraw = true;
                } else {
                    self.open_folder = None;

                    if self.config.enable_bounce_on_launch {
                        self.animations.start_bounce(dragging_idx, 20.0, self.config.launch_bounce_duration_ms);
                    }
                    let (reload, pid_opt) = self.launcher.launch_app(dragging_idx);
                    if let Some(pid) = pid_opt {
                        self.proc_monitor.register(dragging_idx, pid);
                    }
                    if reload { self.reload_config(); }
                }
            } else {
                self.open_folder = None;
                if let Some(target) = self.hovered_app {
                    if target != dragging_idx {
                        self.launcher.reorder_app(dragging_idx, target);
                        self.resize_dock();
                    }
                }
            }
            self.dragging_app = None;
            self.drag_offset_x = 0.0;
            self.needs_redraw = true;
        }
    }

    fn update_hover(&mut self, x: f64, y: f64) {
        self.mouse_x = x;
        self.mouse_y = y;

        let panel_start_y = OVERDRAW as f64;

        if self.config.auto_hide && self.config.smart_hide && !self.dock_revealed {
            if y >= panel_start_y {
                let dy = (y - self.smart_last_y).abs();
                if dy > self.config.smart_hide_y_threshold {
                    self.smart_trigger_start = Some(Instant::now());
                }
                if self.smart_trigger_start.is_none() {
                    self.smart_trigger_start = Some(Instant::now());
                }
                self.smart_last_y = y;
            } else {
                self.smart_trigger_start = None;
                self.smart_last_y = -1.0;
            }
        }

        let visibility = self.animations.get_visibility();

        if let Some(folder_idx) = self.open_folder {
            let hfa = self.renderer.folder_app_at(
                folder_idx, x, y,
                &self.launcher.entries, self.width, &self.config,
            );
            if hfa != self.hovered_folder_app {
                self.hovered_folder_app = hfa;
                self.needs_redraw = true;
            }

            if let Some((px, py, pw, ph)) = self.renderer.popup_rect(
                folder_idx, &self.launcher.entries, self.width, &self.config,
            ) {
                if x >= px && x <= px + pw && y >= py && y <= py + ph {
                    return;
                }
            }
        }

        if visibility < 0.3 {
            if self.hovered_app.is_some() { self.hovered_app = None; self.needs_redraw = true; }
            return;
        }

        let n = self.launcher.entries.len() as u32;
        if n == 0 { self.hovered_app = None; return; }

        let total_w = n * (self.config.icon_size + self.config.icon_padding * 2);
        let start_x = (self.width - total_w) / 2;
        let panel_y = OVERDRAW as f64;

        let mut new_hover = None;
        for i in 0..n {
            let ix = start_x + i * (self.config.icon_size + self.config.icon_padding * 2);
            let iy = panel_y + (self.config.dock_height - self.config.icon_size) as f64 / 2.0;
            if x >= ix as f64
                && x <= (ix + self.config.icon_size + self.config.icon_padding * 2) as f64
                && y >= iy
                && y <= iy + self.config.icon_size as f64
            {
                new_hover = Some(i as usize);
                break;
            }
        }
        if new_hover != self.hovered_app {
            self.hovered_app = new_hover;
            self.needs_redraw = true;
        }
    }
}

// ── Smithay-client-toolkit impls ─────────────────────────────────────────────

impl CompositorHandler for DockApp {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.frame_pending = false;
        self.update_animations();
        if self.needs_redraw && !self.frame_pending {
            self.needs_redraw = false;
            self.draw();
        }
    }

    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for DockApp {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for DockApp {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) { self.exit = true; }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface,
                 configure: LayerSurfaceConfigure, _: u32) {
        self.width  = configure.new_size.0.max(1);
        self.height = configure.new_size.1.max(self.config.dock_height + OVERDRAW);
        self.configured = true;
        if let Some(pool) = &mut self.pool {
            pool.resize((self.width * self.height * 4) as usize).expect("resize pool");
        }
        self.needs_redraw = true;
    }
}

impl SeatHandler for DockApp {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Pointer {
            if let Some(p) = self.pointer.take() { p.release(); }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for DockApp {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.is_mouse_inside = true;
                    self.mouse_leave_time = None;
                    if !self.config.smart_hide || !self.config.auto_hide {
                        self.dock_revealed = true;
                    } else {
                        self.smart_last_y = event.position.1;
                    }
                    self.needs_redraw = true;
                }
                PointerEventKind::Leave { .. } => {
                    self.is_mouse_inside = false;
                    self.dock_revealed = false;
                    self.smart_trigger_start = None;
                    self.smart_last_y = -1.0;
                    self.mouse_leave_time = Some(Instant::now());
                    self.hovered_app = None;
                    self.hovered_folder_app = None;
                    if self.open_folder.is_some() {
                        self.open_folder = None;
                    }
                    self.needs_redraw = true;
                }
                PointerEventKind::Motion { .. } => {
                    if self.dragging_app.is_some() {
                        self.drag_offset_x = event.position.0 - self.drag_start_x;
                        self.needs_redraw = true;
                    }
                    self.update_hover(event.position.0, event.position.1);
                }
                PointerEventKind::Press { button, .. } if button == 0x110 => {
                    self.handle_press(event.position.0, event.position.1);
                }
                PointerEventKind::Release { button, .. } if button == 0x110 => {
                    self.handle_release(event.position.0, event.position.1);
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for DockApp {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm_state }
}

impl wayland_client::Dispatch<wl_region::WlRegion, ()> for DockApp {
    fn event(_: &mut Self, _: &wl_region::WlRegion, _: wl_region::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl ProvidesRegistryState for DockApp {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(DockApp);
delegate_output!(DockApp);
delegate_shm!(DockApp);
delegate_seat!(DockApp);
delegate_pointer!(DockApp);
delegate_layer!(DockApp);
delegate_registry!(DockApp);

// ── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Handle CLI commands
    if args.len() >= 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                println!("rusty-dock - A Wayland dock application");
                println!();
                println!("Usage:");
                println!("  rusty-dock              Start the dock");
                println!("  rusty-dock --config     Open configuration window");
                println!("  rusty-dock --export <path>  Export config to a file");
                println!("  rusty-dock --import <path>  Import config from a file");
                println!("  rusty-dock --help       Show this help message");
                return Ok(());
            }
            "--export" => {
                if args.len() < 3 {
                    eprintln!("Error: --export requires a path argument");
                    eprintln!("Usage: rusty-dock --export <path>");
                    std::process::exit(1);
                }
                let config = DockConfig::load().unwrap_or_default();
                let path = std::path::PathBuf::from(&args[2]);
                config.export_to_path(&path)?;
                println!("✓ Config exported to {:?}", path);
                return Ok(());
            }
            "--import" => {
                if args.len() < 3 {
                    eprintln!("Error: --import requires a path argument");
                    eprintln!("Usage: rusty-dock --import <path>");
                    std::process::exit(1);
                }
                let path = std::path::PathBuf::from(&args[2]);
                let config = DockConfig::import_from_path(&path)?;
                config.save()?;
                println!("✓ Config imported from {:?} and saved", path);
                return Ok(());
            }
            _ => {}
        }
    }

    if std::env::args().any(|a| a == "--config") {
        config_window::run_config_window_blocking();
        return Ok(());
    }

    println!("Starting Rusty Dock…");
    if !imagemagick_probe() {
        return Err("ImageMagick (convert) not found — please install it".into());
    }

    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<DockApp> = EventLoop::try_new()?;
    let registry_state  = RegistryState::new(&globals);
    let seat_state      = SeatState::new(&globals, &qh);
    let output_state    = OutputState::new(&globals, &qh);
    let compositor_state = CompositorState::bind(&globals, &qh)?;
    let shm_state       = Shm::bind(&globals, &qh)?;
    let layer_shell     = LayerShell::bind(&globals, &qh)?;
    let loop_handle     = event_loop.handle();

    let mut app = DockApp::new(
        registry_state, seat_state, output_state,
        compositor_state, shm_state, layer_shell, loop_handle,
    );
    app.init_surface(&qh);

    WaylandSource::new(conn, event_queue).insert(event_loop.handle())?;

    loop {
        app.check_reload_signal();
        app.poll_processes();
        app.tick_smart_hide();

        if app.exit {
            break;
        }

        event_loop.dispatch(None, &mut app)?;

        if app.needs_redraw && !app.frame_pending {
            app.needs_redraw = false;
            app.draw();
        }
    }

    Ok(())
}
