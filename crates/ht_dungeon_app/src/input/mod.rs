use crate::camera::Camera;
use ht_dungeon_core::commands::PlayerCommand;
use ht_dungeon_core::entity::Vec2;
use winit::event::{ElementState, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};

const RIGHT_DRAG_THRESHOLD_PX: f32 = 4.0;

#[derive(Default)]
pub struct InputState {
    pub w_held: bool,
    pub a_held: bool,
    pub s_held: bool,
    pub d_held: bool,
    pub q_held: bool,
    pub e_held: bool,
    pub cursor_pos: (f32, f32),
    pub screen_size: (f32, f32),
    right_mouse_held: bool,
    right_drag_start: (f32, f32),
    right_drag_last: (f32, f32),
    right_drag_moved: bool,
}

impl InputState {
    pub fn on_key(&mut self, key: PhysicalKey, state: ElementState) -> Option<PlayerCommand> {
        let pressed = state == ElementState::Pressed;
        match key {
            PhysicalKey::Code(KeyCode::KeyW) => {
                self.w_held = pressed;
                None
            }
            PhysicalKey::Code(KeyCode::KeyA) => {
                self.a_held = pressed;
                None
            }
            PhysicalKey::Code(KeyCode::KeyS) => {
                self.s_held = pressed;
                None
            }
            PhysicalKey::Code(KeyCode::KeyD) => {
                self.d_held = pressed;
                None
            }
            PhysicalKey::Code(KeyCode::KeyQ) => {
                self.q_held = pressed;
                None
            }
            PhysicalKey::Code(KeyCode::KeyE) => {
                self.e_held = pressed;
                None
            }
            PhysicalKey::Code(KeyCode::Space) if pressed => Some(PlayerCommand::TogglePause),
            PhysicalKey::Code(KeyCode::KeyI) if pressed => Some(PlayerCommand::ToggleInventory),
            PhysicalKey::Code(KeyCode::Escape) if pressed => Some(PlayerCommand::TogglePause),
            PhysicalKey::Code(KeyCode::KeyC) if pressed => None, // handled in update_camera
            _ => None,
        }
    }

    pub fn on_cursor_moved(&mut self, position: (f32, f32), camera: &mut Camera) {
        self.cursor_pos = position;

        if !self.right_mouse_held || self.screen_size.0 <= 0.0 || self.screen_size.1 <= 0.0 {
            return;
        }

        let dx = position.0 - self.right_drag_last.0;
        let dy = position.1 - self.right_drag_last.1;
        if dx != 0.0 || dy != 0.0 {
            let aspect = self.screen_size.0 / self.screen_size.1;
            let world_dx = dx / self.screen_size.0 * 2.0 * camera.half_w(aspect);
            let world_dy = dy / self.screen_size.1 * 2.0 * camera.half_h();
            camera.pan(-world_dx, -world_dy);
        }

        let total_dx = position.0 - self.right_drag_start.0;
        let total_dy = position.1 - self.right_drag_start.1;
        if total_dx * total_dx + total_dy * total_dy
            >= RIGHT_DRAG_THRESHOLD_PX * RIGHT_DRAG_THRESHOLD_PX
        {
            self.right_drag_moved = true;
        }
        self.right_drag_last = position;
    }

    pub fn on_mouse_button(
        &mut self,
        button: MouseButton,
        state: ElementState,
        camera: &Camera,
        ui_consumed: bool,
    ) -> Option<PlayerCommand> {
        if button == MouseButton::Right {
            return self.on_right_mouse_button(state, camera, ui_consumed);
        }

        if ui_consumed || state != ElementState::Pressed {
            return None;
        }

        let (wx, wy) = camera.screen_to_world(
            self.cursor_pos.0,
            self.cursor_pos.1,
            self.screen_size.0,
            self.screen_size.1,
        );
        let world_pos = Vec2::new(wx, wy);
        match button {
            MouseButton::Left => Some(PlayerCommand::LeftClick { world_pos }),
            _ => None,
        }
    }

    fn on_right_mouse_button(
        &mut self,
        state: ElementState,
        camera: &Camera,
        ui_consumed: bool,
    ) -> Option<PlayerCommand> {
        match state {
            ElementState::Pressed if !ui_consumed => {
                self.right_mouse_held = true;
                self.right_drag_start = self.cursor_pos;
                self.right_drag_last = self.cursor_pos;
                self.right_drag_moved = false;
                None
            }
            ElementState::Pressed => {
                self.right_mouse_held = false;
                None
            }
            ElementState::Released => {
                let was_held = self.right_mouse_held;
                let was_drag = self.right_drag_moved;
                self.right_mouse_held = false;

                if ui_consumed || !was_held || was_drag {
                    return None;
                }

                let (wx, wy) = camera.screen_to_world(
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    self.screen_size.0,
                    self.screen_size.1,
                );
                Some(PlayerCommand::RightClick {
                    world_pos: Vec2::new(wx, wy),
                })
            }
        }
    }

    pub fn update_camera(&self, camera: &mut Camera, dt: f32, _hero_pos: Option<(f32, f32)>) {
        let pan_speed = camera.zoom * 0.8 * dt;
        if self.w_held {
            camera.pan(0.0, -pan_speed);
        }
        if self.s_held {
            camera.pan(0.0, pan_speed);
        }
        if self.a_held {
            camera.pan(-pan_speed, 0.0);
        }
        if self.d_held {
            camera.pan(pan_speed, 0.0);
        }
        if self.q_held {
            camera.zoom_out();
        }
        if self.e_held {
            camera.zoom_in();
        }
    }
}
