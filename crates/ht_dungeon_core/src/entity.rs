use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, Neg};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct EntityId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ItemId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    pub fn normalized(self) -> Self {
        let len = self.length();
        if len > 1e-6 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::ZERO
        }
    }

    pub fn distance_to(self, other: Vec2) -> f32 {
        (other - self).length()
    }

    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn to_world_center(self) -> Vec2 {
        Vec2::new(self.x as f32 + 0.5, self.y as f32 + 0.5)
    }
}

pub fn world_to_tile(pos: Vec2) -> TileCoord {
    TileCoord::new(pos.x.floor() as i32, pos.y.floor() as i32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Hero,
    Rat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Player,
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RatState {
    Idle,
    ChasingHero,
    AttackingHero,
    Dead,
}

pub struct RatAiState {
    pub state: RatState,
    pub lost_sight_timer: f32,
}

pub struct MovementState {
    pub speed: f32,
    pub path: Vec<Vec2>,
    pub path_index: usize,
    pub velocity: Vec2,
    pub attack_target: Option<EntityId>,
    pub is_in_attack_range: bool,
}

impl MovementState {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            path: Vec::new(),
            path_index: 0,
            velocity: Vec2::ZERO,
            attack_target: None,
            is_in_attack_range: false,
        }
    }

    pub fn clear_path(&mut self) {
        self.path.clear();
        self.path_index = 0;
        self.velocity = Vec2::ZERO;
        self.attack_target = None;
        self.is_in_attack_range = false;
    }

    pub fn set_path(&mut self, path: Vec<Vec2>) {
        self.path = path;
        self.path_index = 0;
        self.is_in_attack_range = false;
    }

    pub fn current_waypoint(&self) -> Option<Vec2> {
        self.path.get(self.path_index).copied()
    }

    pub fn advance_waypoint(&mut self) {
        self.path_index += 1;
    }

    pub fn has_path(&self) -> bool {
        self.path_index < self.path.len()
    }
}

pub struct CombatState {
    pub attack_damage_min: i32,
    pub attack_damage_max: i32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub cooldown_remaining: f32,
    pub target: Option<EntityId>,
}

impl CombatState {
    pub fn ready(&self) -> bool {
        self.cooldown_remaining <= 0.0
    }

    pub fn start_cooldown(&mut self) {
        self.cooldown_remaining = self.attack_cooldown;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.cooldown_remaining > 0.0 {
            self.cooldown_remaining -= dt;
        }
    }
}

pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub position: Vec2,
    pub radius: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub faction: Faction,
    pub movement: MovementState,
    pub combat: CombatState,
    pub rat_ai: Option<RatAiState>,
    pub alive: bool,
}

impl Entity {
    pub fn new_hero(id: EntityId, position: Vec2) -> Self {
        Self {
            id,
            kind: EntityKind::Hero,
            position,
            radius: 0.30,
            hp: 10,
            max_hp: 10,
            faction: Faction::Player,
            movement: MovementState::new(3.0),
            combat: CombatState {
                attack_damage_min: 1,
                attack_damage_max: 2,
                attack_range: 0.75,
                attack_cooldown: 0.7,
                cooldown_remaining: 0.0,
                target: None,
            },
            rat_ai: None,
            alive: true,
        }
    }

    pub fn new_rat(id: EntityId, position: Vec2) -> Self {
        Self {
            id,
            kind: EntityKind::Rat,
            position,
            radius: 0.25,
            hp: 1,
            max_hp: 1,
            faction: Faction::Enemy,
            movement: MovementState::new(2.0),
            combat: CombatState {
                attack_damage_min: 1,
                attack_damage_max: 1,
                attack_range: 0.65,
                attack_cooldown: 1.0,
                cooldown_remaining: 0.0,
                target: None,
            },
            rat_ai: Some(RatAiState {
                state: RatState::Idle,
                lost_sight_timer: 0.0,
            }),
            alive: true,
        }
    }

    pub fn take_damage(&mut self, amount: i32) {
        self.hp -= amount;
        if self.hp <= 0 {
            self.hp = 0;
            self.alive = false;
            if let Some(ai) = &mut self.rat_ai {
                ai.state = RatState::Dead;
            }
        }
    }

    pub fn attack_enter_range(&self) -> f32 {
        self.radius + self.combat.attack_range
    }

    pub fn attack_exit_range(&self, target_radius: f32) -> f32 {
        self.radius + target_radius + self.combat.attack_range + 0.20
    }

    pub fn stop_distance(&self, target_radius: f32) -> f32 {
        self.radius + target_radius + self.combat.attack_range
    }
}
