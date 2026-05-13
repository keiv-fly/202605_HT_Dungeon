use crate::entity::{EntityId, ItemId, Vec2};

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    SetPaused(bool),
    LeftClick { world_pos: Vec2 },
    RightClick { world_pos: Vec2 },
    AttackEnemy { enemy_id: EntityId },
    PickUpItem { item_id: ItemId },
    TogglePause,
    ToggleInventory,
}
