mod camera;
mod input;
mod render;
mod ui;

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use winit::application::ApplicationHandler;
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use ht_dungeon_core::commands::PlayerCommand;
use ht_dungeon_core::snapshot::RenderSnapshot;
use ht_dungeon_core::world::GameWorld;

use camera::Camera;
use input::InputState;

const TICK_RATE: u64 = 60;
const TICK_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TICK_RATE);

struct RenderState {
    window: Arc<Window>,
    renderer: render::Renderer,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    camera: Camera,
    input: InputState,
    snapshot: Option<RenderSnapshot>,
    initial_camera_centered: bool,
    command_tx: Sender<PlayerCommand>,
    snapshot_rx: Receiver<RenderSnapshot>,
    last_frame: Instant,
}

pub struct App {
    state: Option<RenderState>,
}

impl App {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("HT Dungeon")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32)),
                )
                .unwrap(),
        );

        let renderer = pollster::block_on(render::Renderer::new(Arc::clone(&window)));

        let egui_ctx = egui::Context::default();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            None,
            None,
            None,
        );

        let (command_tx, command_rx) = bounded::<PlayerCommand>(256);
        let (snapshot_tx, snapshot_rx) = bounded::<RenderSnapshot>(2);

        let seed: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);

        std::thread::spawn(move || logic_thread(seed, command_rx, snapshot_tx));

        let camera = Camera::new(40.0, 30.0);
        let size = window.inner_size();
        let mut input = InputState::default();
        input.screen_size = (size.width as f32, size.height as f32);

        self.state = Some(RenderState {
            window,
            renderer,
            egui_ctx,
            egui_winit,
            camera,
            input,
            snapshot: None,
            initial_camera_centered: false,
            command_tx,
            snapshot_rx,
            last_frame: Instant::now(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        let egui_consumed = state
            .egui_winit
            .on_window_event(state.window.as_ref(), &event)
            .consumed;

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                state.renderer.resize(size);
                state.input.screen_size = (size.width as f32, size.height as f32);
            }

            WindowEvent::KeyboardInput {
                event: key_event, ..
            } if !egui_consumed => {
                if let Some(cmd) = state.input.on_key(key_event.physical_key, key_event.state) {
                    let _ = state.command_tx.try_send(cmd);
                }

                // Center camera on hero with C
                use winit::event::ElementState;
                use winit::keyboard::{KeyCode, PhysicalKey};
                if key_event.physical_key == PhysicalKey::Code(KeyCode::KeyC)
                    && key_event.state == ElementState::Pressed
                {
                    if let Some(snap) = &state.snapshot {
                        state.camera.x = snap.hero_status.position.x;
                        state.camera.y = snap.hero_status.position.y;
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                state
                    .input
                    .on_cursor_moved((position.x as f32, position.y as f32), &mut state.camera);
            }

            WindowEvent::MouseInput {
                button,
                state: btn_state,
                ..
            } => {
                if let Some(cmd) =
                    state
                        .input
                        .on_mouse_button(button, btn_state, &state.camera, egui_consumed)
                {
                    let _ = state.command_tx.try_send(cmd);
                }
            }

            WindowEvent::MouseWheel { delta, .. } if !egui_consumed => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.01,
                };
                state.camera.zoom_scroll(scroll);
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame).as_secs_f32().min(0.1);
                state.last_frame = now;

                // Drain newest snapshot
                while let Ok(snap) = state.snapshot_rx.try_recv() {
                    if !state.initial_camera_centered {
                        state.camera.x = snap.hero_status.position.x;
                        state.camera.y = snap.hero_status.position.y;
                        state.initial_camera_centered = true;
                    }
                    state.snapshot = Some(snap);
                }

                // Camera pan from held keys
                state.input.update_camera(&mut state.camera, dt, None);

                let raw_input = state.egui_winit.take_egui_input(state.window.as_ref());
                let egui_output = state.egui_ctx.run(raw_input, |ctx| {
                    if let Some(snap) = &state.snapshot {
                        ui::draw_ui(ctx, snap);
                    }
                });
                state.egui_winit.handle_platform_output(
                    state.window.as_ref(),
                    egui_output.platform_output.clone(),
                );

                if let Some(snap) = &state.snapshot {
                    let ppp = state.window.scale_factor() as f32;
                    match state.renderer.render(
                        snap,
                        &state.camera,
                        &state.egui_ctx,
                        egui_output,
                        ppp,
                    ) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let size = state.renderer.size;
                            state.renderer.resize(size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => log::warn!("Surface error: {e:?}"),
                    }
                }

                state.window.request_redraw();
            }

            _ => {}
        }
    }
}

fn logic_thread(
    seed: u64,
    command_rx: Receiver<PlayerCommand>,
    snapshot_tx: Sender<RenderSnapshot>,
) {
    let mut world = GameWorld::new(seed);
    let mut last_tick = Instant::now();

    // Send initial snapshot
    let _ = snapshot_tx.try_send(world.make_snapshot());

    loop {
        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);

        if elapsed >= TICK_DURATION {
            let dt = elapsed.as_secs_f32().min(0.05);
            last_tick = now;

            let mut cmds = Vec::new();
            while let Ok(cmd) = command_rx.try_recv() {
                cmds.push(cmd);
            }

            world.update(dt, &cmds);

            // Try to send; if channel full, drop the snapshot (renderer keeps the prior one)
            let snap = world.make_snapshot();
            match snapshot_tx.try_send(snap) {
                Ok(_) | Err(crossbeam_channel::TrySendError::Full(_)) => {}
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => break,
            }
        } else {
            let sleep = TICK_DURATION - elapsed;
            if sleep > Duration::from_millis(1) {
                std::thread::sleep(sleep - Duration::from_millis(1));
            } else {
                std::hint::spin_loop();
            }
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
