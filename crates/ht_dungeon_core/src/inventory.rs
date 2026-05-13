use std::collections::HashMap;
use crate::entity::{ItemId, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    RatTail,
}

#[derive(Default)]
pub struct Inventory {
    pub stacks: HashMap<ItemKind, u32>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, kind: ItemKind, count: u32) {
        *self.stacks.entry(kind).or_insert(0) += count;
    }

    pub fn count(&self, kind: ItemKind) -> u32 {
        self.stacks.get(&kind).copied().unwrap_or(0)
    }
}

pub struct GroundItem {
    pub id: ItemId,
    pub kind: ItemKind,
    pub count: u32,
    pub position: Vec2,
}

pub const PICKUP_RADIUS: f32 = 0.5;
pub const RAT_SIGHT_RANGE: f32 = 7.0;
