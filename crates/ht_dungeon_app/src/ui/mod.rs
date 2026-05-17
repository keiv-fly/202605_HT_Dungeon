use egui::Context;
use ht_dungeon_core::snapshot::{GameState, RenderSnapshot};

#[derive(Default)]
pub struct DebugUiState {
    pub visible: bool,
    command_input: String,
    last_message: Option<String>,
}

pub enum DebugCommand {
    ToggleHeroCircle,
}

pub fn draw_ui(
    ctx: &Context,
    snapshot: &RenderSnapshot,
    debug: &mut DebugUiState,
    show_hero_circle: bool,
) -> Vec<DebugCommand> {
    let mut commands = Vec::new();

    draw_hud(ctx, snapshot);

    if snapshot.inventory.show_panel {
        draw_inventory(ctx, snapshot);
    }

    if let Some(info) = &snapshot.inspect_info {
        draw_inspect(ctx, info);
    }

    match snapshot.game_state {
        GameState::GameOver => draw_game_over(ctx),
        GameState::Victory => draw_victory(ctx, snapshot.inventory.rat_tails),
        _ => {}
    }

    if debug.visible {
        draw_debug(ctx, snapshot, debug, show_hero_circle, &mut commands);
    }

    commands
}

fn draw_hud(ctx: &Context, snapshot: &RenderSnapshot) {
    egui::Window::new("HUD")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
        .show(ctx, |ui| {
            let hp = snapshot.hero_status.hp;
            let max = snapshot.hero_status.max_hp;
            ui.label(format!("HP: {hp} / {max}"));
            ui.label(format!("Weapon: {}", snapshot.hero_status.weapon_name));
            if snapshot.inventory.rat_tails > 0 {
                ui.label(format!("Rat Tail x {}", snapshot.inventory.rat_tails));
            }
            if snapshot.paused {
                ui.colored_label(egui::Color32::YELLOW, "PAUSED");
            }
        });
}

fn draw_inventory(ctx: &Context, snapshot: &RenderSnapshot) {
    egui::Window::new("Inventory")
        .resizable(false)
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .show(ctx, |ui| {
            ui.label("──────────");
            let tails = snapshot.inventory.rat_tails;
            if tails > 0 {
                ui.label(format!("Rat Tail: {tails}"));
            } else {
                ui.label("(empty)");
            }
        });
}

fn draw_inspect(ctx: &Context, info: &ht_dungeon_core::snapshot::InspectInfo) {
    egui::Window::new(&info.title)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-8.0, -8.0])
        .show(ctx, |ui| {
            for line in &info.lines {
                ui.label(line);
            }
        });
}

fn draw_game_over(ctx: &Context) {
    egui::Window::new("Game Over")
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("You have died.");
            ui.label("Restart the game to try again.");
        });
}

fn draw_victory(ctx: &Context, tails: u32) {
    egui::Window::new("Victory!")
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("All rats slain!");
            ui.label(format!("Rat Tails collected: {tails}"));
        });
}

fn draw_debug(
    ctx: &Context,
    snapshot: &RenderSnapshot,
    debug: &mut DebugUiState,
    show_hero_circle: bool,
    commands: &mut Vec<DebugCommand>,
) {
    egui::Window::new("Debug")
        .default_pos([8.0, 120.0])
        .show(ctx, |ui| {
            ui.label("Commands");
            ui.monospace("/show_circle");
            ui.separator();
            ui.label(format!("Hero circle: {}", on_off(show_hero_circle)));
            ui.label(format!("Frame: {}", snapshot.frame_id));
            ui.label(format!("Sim time: {:.2}", snapshot.sim_time));

            let response = ui.text_edit_singleline(&mut debug.command_input);
            let submitted =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));

            if ui.button("Run").clicked() || submitted {
                submit_debug_command(debug, commands);
            }

            if let Some(message) = &debug.last_message {
                ui.label(message);
            }
        });
}

fn submit_debug_command(debug: &mut DebugUiState, commands: &mut Vec<DebugCommand>) {
    let command = debug.command_input.trim();
    match command {
        "/show_circle" => {
            commands.push(DebugCommand::ToggleHeroCircle);
            debug.last_message = Some("Toggled hero circle.".to_owned());
            debug.command_input.clear();
        }
        "" => {}
        _ => {
            debug.last_message = Some(format!("Unknown command: {command}"));
        }
    }
}

fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}
