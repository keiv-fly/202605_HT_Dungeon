use winit::event::{ElementState, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use ht_dungeon_core::commands::PlayerCommand;
use ht_dungeon_core::entity::Vec2;
use crate::camera::Camera;

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
}

impl InputState {
    pub fn on_key(&mut self, key: PhysicalKey, state: ElementState) -> Option<PlayerCommand> {
        let pressed = state == ElementState::Pressed;
        match key {
            PhysicalKey::Code(KeyCode::KeyW) => { self.w_held = pressed; None }
            PhysicalKey::Code(KeyCode::KeyA) => { self.a_held = pressed; None }
            PhysicalKey::Code(KeyCode::KeyS) => { self.s_held = pressed; None }
            PhysicalKey::Code(KeyCode::KeyD) => { self.d_held = pressed; None }
            PhysicalKey::Code(KeyCode::KeyQ) => { self.q_held = pressed; None }
            PhysicalKey::Code(KeyCode::KeyE) => { self.e_held = pressed; None }
            PhysicalKey::Code(KeyCode::Space) if pressed => {
                Some(PlayerCommand::TogglePause)
            }
            PhysicalKey::Code(KeyCode::KeyI) if pressed => {
                Some(PlayerCommand::ToggleInventory)
            }
            PhysicalKey::Code(KeyCode::Escape) if pressed => {
                Some(PlayerCommand::TogglePause)
            }
            PhysicalKey::Code(KeyCode::KeyC) if pressed => None, // handled in update_camera
            _ => None,
        }
    }

    pub fn on_mouse_button(
        &self,
        button: MouseButton,
        state: ElementState,
        camera: &Camera,
    ) -> Option<PlayerCommand> {
        if state != ElementState::Pressed {
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
            MouseButton::Right => Some(PlayerCommand::RightClick { world_pos }),
            _ => None,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera, dt: f32, _hero_pos: Option<(f32, f32)>) {
        let pan_speed = camera.zoom * 0.8 * dt;
        if self.w_held { camera.pan(0.0, -pan_speed); }
        if self.s_held { camera.pan(0.0,  pan_speed); }
        if self.a_held { camera.pan(-pan_speed, 0.0); }
        if self.d_held { camera.pan( pan_speed, 0.0); }
        if self.q_held { camera.zoom_out(); }
        if self.e_held { camera.zoom_in(); }
    }

}
