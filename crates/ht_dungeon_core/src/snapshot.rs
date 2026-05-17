use crate::config::GameConfig;
use crate::dungeon::TileKind;
use crate::entity::{EntityId, EntityKind, ItemId, Vec2};
use crate::inventory::ItemKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cy: i32,
}

pub const CHUNK_SIZE: i32 = 16;

#[derive(Clone, Debug)]
pub struct TileRenderData {
    pub x: i32,
    pub y: i32,
    pub kind: TileKind,
}

#[derive(Clone, Debug)]
pub struct AttackAnimationRenderData {
    pub direction: Vec2,
    pub elapsed: f32,
}

#[derive(Clone, Debug)]
pub struct EntityRenderData {
    pub id: EntityId,
    pub kind: EntityKind,
    pub position: Vec2,
    pub hp: i32,
    pub max_hp: i32,
    pub alive: bool,
    pub attack_animation: Option<AttackAnimationRenderData>,
}

#[derive(Clone, Debug)]
pub struct ItemRenderData {
    pub id: ItemId,
    pub kind: ItemKind,
    pub position: Vec2,
}

#[derive(Clone, Debug)]
pub struct HeroStatus {
    pub hp: i32,
    pub max_hp: i32,
    pub weapon_name: &'static str,
    pub position: Vec2,
}

#[derive(Clone, Debug)]
pub struct InventoryView {
    pub rat_tails: u32,
    pub show_panel: bool,
}

#[derive(Clone, Debug)]
pub struct InspectInfo {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Loading,
    Running,
    Paused,
    GameOver,
    Victory,
}

#[derive(Clone, Debug)]
pub struct RenderSnapshot {
    pub frame_id: u64,
    pub sim_time: f64,
    pub config: GameConfig,
    pub paused: bool,
    pub map_width: u32,
    pub map_height: u32,
    pub tiles: Vec<TileRenderData>,
    pub dirty_chunks: Vec<ChunkCoord>,
    pub entities: Vec<EntityRenderData>,
    pub items: Vec<ItemRenderData>,
    pub hero_status: HeroStatus,
    pub inventory: InventoryView,
    pub inspect_info: Option<InspectInfo>,
    pub game_state: GameState,
    pub rng_seed: u64,
}
