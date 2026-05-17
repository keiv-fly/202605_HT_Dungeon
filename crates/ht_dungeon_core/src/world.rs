use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::commands::PlayerCommand;
use crate::dungeon::{
    load_standard_dungeon, DungeonSpawn, DungeonSpawnKind, Room, TileKind, TileMap,
};
use crate::entity::{world_to_tile, Entity, EntityId, EntityKind, Faction, ItemId, RatState, Vec2};
use crate::inventory::{GroundItem, Inventory, ItemKind, PICKUP_RADIUS, RAT_SIGHT_RANGE};
use crate::pathfinding::{find_path, nearest_walkable};
use crate::snapshot::{
    EntityRenderData, GameState, HeroStatus, InspectInfo, InventoryView, ItemRenderData,
    RenderSnapshot, TileRenderData,
};

pub struct GameWorld {
    pub rng_seed: u64,
    pub tick_count: u64,
    pub sim_time: f64,
    pub state: GameState,
    pub map: TileMap,
    pub rooms: Vec<Room>,
    pub initial_spawns: Vec<DungeonSpawn>,
    pub hero_id: EntityId,
    pub entities: Vec<Entity>,
    pub items: Vec<GroundItem>,
    pub inventory: Inventory,
    pub inspect_info: Option<InspectInfo>,
    pub show_inventory: bool,
    rng: ChaCha8Rng,
    next_entity_id: u32,
    next_item_id: u32,
    frame_id: u64,
}

impl GameWorld {
    pub fn new(seed: u64) -> Self {
        let dungeon = load_standard_dungeon();
        let mut world = Self {
            rng_seed: seed,
            tick_count: 0,
            sim_time: 0.0,
            state: GameState::Running,
            map: dungeon.map,
            rooms: dungeon.rooms,
            initial_spawns: dungeon.spawns,
            hero_id: EntityId(0),
            entities: Vec::new(),
            items: Vec::new(),
            inventory: Inventory::new(),
            inspect_info: None,
            show_inventory: false,
            rng: ChaCha8Rng::seed_from_u64(seed.wrapping_add(1)),
            next_entity_id: 0,
            next_item_id: 0,
            frame_id: 0,
        };
        world.spawn_initial_entities();
        world
    }

    fn alloc_entity_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    fn alloc_item_id(&mut self) -> ItemId {
        let id = ItemId(self.next_item_id);
        self.next_item_id += 1;
        id
    }

    fn spawn_initial_entities(&mut self) {
        if !self.initial_spawns.is_empty() {
            self.spawn_map_entities();
            return;
        }

        let hero_room = self.rooms[0].clone();
        let (hx, hy) = hero_room.center();
        let hero_pos = Vec2::new(hx as f32 + 0.5, hy as f32 + 0.5);
        let hero_id = self.alloc_entity_id();
        self.hero_id = hero_id;
        self.entities.push(Entity::new_hero(hero_id, hero_pos));

        let rooms = self.rooms.clone();
        for (i, room) in rooms.iter().enumerate() {
            let count = match i {
                0 | 1 => 1,
                _ => {
                    if self.rng.gen_bool(0.5) {
                        2
                    } else {
                        1
                    }
                }
            };
            for _ in 0..count {
                self.try_spawn_rat(room, hero_pos);
            }
        }
    }

    fn spawn_map_entities(&mut self) {
        let hero_spawn = self
            .initial_spawns
            .iter()
            .find(|spawn| spawn.kind == DungeonSpawnKind::Hero)
            .copied()
            .expect("starting dungeon map must contain a hero spawn");
        let (hx, hy) = hero_spawn.center_f();
        let hero_pos = Vec2::new(hx, hy);
        let hero_id = self.alloc_entity_id();
        self.hero_id = hero_id;
        self.entities.push(Entity::new_hero(hero_id, hero_pos));

        let rat_spawns: Vec<DungeonSpawn> = self
            .initial_spawns
            .iter()
            .filter(|spawn| spawn.kind == DungeonSpawnKind::Rat)
            .copied()
            .collect();
        for spawn in rat_spawns {
            let (rx, ry) = spawn.center_f();
            let id = self.alloc_entity_id();
            self.entities.push(Entity::new_rat(id, Vec2::new(rx, ry)));
        }
    }

    fn try_spawn_rat(&mut self, room: &Room, hero_pos: Vec2) {
        let min_spawn_dist = 1.5f32;
        for _ in 0..20 {
            let (rx, ry) = room.random_interior_point(&mut self.rng);
            let pos = Vec2::new(rx as f32 + 0.5, ry as f32 + 0.5);
            if pos.distance_to(hero_pos) < min_spawn_dist {
                continue;
            }
            let occupied = self
                .entities
                .iter()
                .any(|e| e.alive && e.position.distance_to(pos) < 0.8);
            if occupied {
                continue;
            }
            if !self.map.is_walkable(rx, ry) {
                continue;
            }
            let id = self.alloc_entity_id();
            self.entities.push(Entity::new_rat(id, pos));
            return;
        }
    }

    pub fn update(&mut self, dt: f32, commands: &[PlayerCommand]) {
        for cmd in commands {
            self.process_command(cmd.clone());
        }

        let paused = matches!(self.state, GameState::Paused);
        if !paused && matches!(self.state, GameState::Running | GameState::Paused) {
            if !matches!(self.state, GameState::GameOver | GameState::Victory) {
                self.tick(dt);
            }
        }
        self.frame_id += 1;
    }

    fn tick(&mut self, dt: f32) {
        self.sim_time += dt as f64;
        self.tick_count += 1;

        self.update_combat_cooldowns(dt);
        self.update_rat_ai(dt);
        self.update_movement(dt);
        self.resolve_actor_separation();
        self.wall_collision();
        self.execute_attacks();
        self.check_pickup();
        self.check_game_state();
    }

    fn process_command(&mut self, cmd: PlayerCommand) {
        match cmd {
            PlayerCommand::TogglePause => {
                self.state = match self.state {
                    GameState::Running => GameState::Paused,
                    GameState::Paused => GameState::Running,
                    other => other,
                };
            }
            PlayerCommand::SetPaused(p) => {
                if matches!(self.state, GameState::Running | GameState::Paused) {
                    self.state = if p {
                        GameState::Paused
                    } else {
                        GameState::Running
                    };
                }
            }
            PlayerCommand::ToggleInventory => {
                self.show_inventory = !self.show_inventory;
            }
            PlayerCommand::LeftClick { world_pos } => {
                self.handle_left_click(world_pos);
            }
            PlayerCommand::RightClick { world_pos } => {
                self.handle_right_click(world_pos);
            }
            PlayerCommand::AttackEnemy { enemy_id } => {
                self.order_hero_attack(enemy_id);
            }
            PlayerCommand::PickUpItem { item_id } => {
                self.order_hero_pickup(item_id);
            }
        }
    }

    fn handle_left_click(&mut self, world_pos: Vec2) {
        let tc = world_to_tile(world_pos);

        // Check entity hit (rat)
        if let Some(enemy_id) = self.entity_at_pos(world_pos, Faction::Enemy) {
            self.order_hero_attack(enemy_id);
            return;
        }

        // Check item hit
        if let Some(item_id) = self.item_at_pos(world_pos) {
            self.order_hero_pickup(item_id);
            return;
        }

        // Move toward clicked position
        let target = if self.map.is_walkable(tc.x, tc.y) {
            Some(world_pos)
        } else {
            nearest_walkable(&self.map, world_pos)
        };

        if let Some(goal) = target {
            let hero_pos = self.hero().position;
            let hero_radius = self.hero().radius;
            if let Some(path) = find_path(&self.map, hero_pos, goal, hero_radius) {
                let hero = self.hero_mut();
                hero.movement.attack_target = None;
                hero.movement.set_path(path);
                hero.combat.target = None;
            }
        }
    }

    fn handle_right_click(&mut self, world_pos: Vec2) {
        let tc = world_to_tile(world_pos);
        self.inspect_info = Some(self.build_inspect_info(world_pos, tc));
    }

    fn build_inspect_info(&self, world_pos: Vec2, tc: crate::entity::TileCoord) -> InspectInfo {
        // Check entities
        for e in &self.entities {
            if e.position.distance_to(world_pos) < e.radius + 0.3 {
                return match e.kind {
                    EntityKind::Hero => InspectInfo {
                        title: "Hero".to_string(),
                        lines: vec![
                            format!("HP: {} / {}", e.hp, e.max_hp),
                            "Weapon: Dagger".to_string(),
                            format!("Rat Tails: {}", self.inventory.count(ItemKind::RatTail)),
                        ],
                    },
                    EntityKind::Rat => InspectInfo {
                        title: if e.alive { "Rat" } else { "Dead Rat" }.to_string(),
                        lines: vec![
                            format!("HP: {} / {}", e.hp, e.max_hp),
                            "Attack: 1".to_string(),
                            "Hostile.".to_string(),
                            "Drops: Rat Tail".to_string(),
                        ],
                    },
                };
            }
        }
        // Check items
        for item in &self.items {
            if item.position.distance_to(world_pos) < 0.4 {
                return InspectInfo {
                    title: "Rat Tail".to_string(),
                    lines: vec![
                        "A severed rat tail.".to_string(),
                        format!("Count: {}", item.count),
                    ],
                };
            }
        }
        // Tile
        match self.map.tile_kind(tc.x, tc.y) {
            TileKind::Wall => InspectInfo {
                title: "Wall".to_string(),
                lines: vec!["A rough dungeon wall.".to_string(), "HP: 10".to_string()],
            },
            TileKind::Floor => InspectInfo {
                title: "Floor".to_string(),
                lines: vec!["Worn stone floor.".to_string()],
            },
        }
    }

    fn entity_at_pos(&self, world_pos: Vec2, faction: Faction) -> Option<EntityId> {
        self.entities
            .iter()
            .filter(|e| e.alive && e.faction == faction)
            .find(|e| e.position.distance_to(world_pos) < e.radius + 0.2)
            .map(|e| e.id)
    }

    fn item_at_pos(&self, world_pos: Vec2) -> Option<ItemId> {
        self.items
            .iter()
            .find(|i| i.position.distance_to(world_pos) < 0.4)
            .map(|i| i.id)
    }

    fn order_hero_attack(&mut self, enemy_id: EntityId) {
        let enemy_pos = match self.entities.iter().find(|e| e.id == enemy_id && e.alive) {
            Some(e) => e.position,
            None => return,
        };
        let hero_pos = self.hero().position;
        let target_radius = self
            .entities
            .iter()
            .find(|e| e.id == enemy_id)
            .map_or(0.25, |e| e.radius);
        let stop_dist = self.hero().stop_distance(target_radius);

        if hero_pos.distance_to(enemy_pos) > stop_dist + 0.05 {
            let hero_radius = self.hero().radius;
            if let Some(path) = find_path(&self.map, hero_pos, enemy_pos, hero_radius) {
                let hero = self.hero_mut();
                hero.movement.attack_target = Some(enemy_id);
                hero.movement.set_path(path);
            }
        }
        self.hero_mut().combat.target = Some(enemy_id);
    }

    fn order_hero_pickup(&mut self, item_id: ItemId) {
        let item_pos = match self.items.iter().find(|i| i.id == item_id) {
            Some(i) => i.position,
            None => return,
        };
        let hero_pos = self.hero().position;
        let hero_radius = self.hero().radius;
        if let Some(path) = find_path(&self.map, hero_pos, item_pos, hero_radius) {
            let hero = self.hero_mut();
            hero.movement.attack_target = None;
            hero.movement.set_path(path);
        }
    }

    fn update_combat_cooldowns(&mut self, dt: f32) {
        for e in &mut self.entities {
            e.combat.tick(dt);
        }
    }

    fn update_rat_ai(&mut self, dt: f32) {
        let hero_pos = self.hero().position;
        let hero_id = self.hero_id;
        let map_ref = &self.map;

        let entity_indices: Vec<usize> = (0..self.entities.len())
            .filter(|&i| self.entities[i].kind == EntityKind::Rat && self.entities[i].alive)
            .collect();

        for i in entity_indices {
            let dist = self.entities[i].position.distance_to(hero_pos);
            let can_see = dist <= RAT_SIGHT_RANGE
                && line_of_sight(map_ref, self.entities[i].position, hero_pos);

            let state = &self.entities[i].rat_ai.as_ref().unwrap().state;
            let new_state = match state {
                RatState::Idle => {
                    if can_see {
                        RatState::ChasingHero
                    } else {
                        RatState::Idle
                    }
                }
                RatState::ChasingHero => {
                    let stop = self.entities[i].stop_distance(0.30);
                    if dist <= stop {
                        RatState::AttackingHero
                    } else if can_see {
                        RatState::ChasingHero
                    } else {
                        let timer = self.entities[i].rat_ai.as_ref().unwrap().lost_sight_timer;
                        if timer > 0.0 {
                            RatState::ChasingHero
                        } else {
                            RatState::Idle
                        }
                    }
                }
                RatState::AttackingHero => {
                    let exit = self.entities[i].attack_exit_range(0.30);
                    if dist > exit {
                        RatState::ChasingHero
                    } else {
                        RatState::AttackingHero
                    }
                }
                RatState::Dead => RatState::Dead,
            };

            if let Some(ai) = &mut self.entities[i].rat_ai {
                if !can_see && matches!(ai.state, RatState::ChasingHero) {
                    ai.lost_sight_timer -= dt;
                } else if can_see {
                    ai.lost_sight_timer = 2.0;
                }
                ai.state = new_state;
            }

            match self.entities[i].rat_ai.as_ref().map(|a| a.state) {
                Some(RatState::ChasingHero) => {
                    let rat_pos = self.entities[i].position;
                    let rat_radius = self.entities[i].radius;
                    let stop = self.entities[i].stop_distance(0.30);
                    if rat_pos.distance_to(hero_pos) > stop {
                        if self.entities[i].movement.attack_target != Some(hero_id) {
                            if let Some(path) = find_path(map_ref, rat_pos, hero_pos, rat_radius) {
                                self.entities[i].movement.attack_target = Some(hero_id);
                                self.entities[i].movement.set_path(path);
                            }
                        }
                    }
                    self.entities[i].combat.target = Some(hero_id);
                }
                Some(RatState::AttackingHero) => {
                    self.entities[i].movement.clear_path();
                    self.entities[i].combat.target = Some(hero_id);
                }
                Some(RatState::Idle) | Some(RatState::Dead) | None => {
                    self.entities[i].movement.clear_path();
                    self.entities[i].combat.target = None;
                }
            }
        }
    }

    fn update_movement(&mut self, dt: f32) {
        let map = &self.map;

        for i in 0..self.entities.len() {
            if !self.entities[i].alive {
                continue;
            }

            let attack_target_id = self.entities[i].movement.attack_target;
            let attack_target_pos = attack_target_id.and_then(|tid| {
                self.entities
                    .iter()
                    .find(|e| e.id == tid && e.alive)
                    .map(|e| e.position)
            });
            let attack_target_radius = attack_target_id
                .and_then(|tid| self.entities.iter().find(|e| e.id == tid).map(|e| e.radius))
                .unwrap_or(0.25);

            let entity = &mut self.entities[i];

            if let Some(target_pos) = attack_target_pos {
                let dist = entity.position.distance_to(target_pos);
                let stop = entity.stop_distance(attack_target_radius);
                let exit = entity.attack_exit_range(attack_target_radius);

                if entity.movement.is_in_attack_range {
                    if dist > exit {
                        entity.movement.is_in_attack_range = false;
                    } else {
                        entity.movement.velocity = Vec2::ZERO;
                        continue;
                    }
                } else if dist <= stop {
                    entity.movement.is_in_attack_range = true;
                    entity.movement.velocity = Vec2::ZERO;
                    continue;
                }
            }

            if let Some(wp) = entity.movement.current_waypoint() {
                let to_wp = wp - entity.position;
                let dist = to_wp.length();
                let arrive_thresh = 0.1;

                if dist < arrive_thresh {
                    entity.movement.advance_waypoint();
                    if !entity.movement.has_path() {
                        entity.movement.velocity = Vec2::ZERO;
                    }
                } else {
                    let dir = to_wp.normalized();
                    let speed = entity.movement.speed;
                    entity.movement.velocity = dir * speed;
                    let step = entity.movement.velocity * dt;
                    entity.position += step;
                    clamp_to_map(map, &mut entity.position, entity.radius);
                }
            } else {
                entity.movement.velocity = Vec2::ZERO;
            }
        }
    }

    fn resolve_actor_separation(&mut self) {
        let n = self.entities.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if !self.entities[i].alive || !self.entities[j].alive {
                    continue;
                }
                let delta = self.entities[i].position - self.entities[j].position;
                let dist = delta.length();
                let min_dist = self.entities[i].radius + self.entities[j].radius;
                if dist > 0.0 && dist < min_dist {
                    let overlap = min_dist - dist;
                    let normal = delta.normalized();
                    self.entities[i].position += normal * (overlap * 0.5);
                    self.entities[j].position -= normal * (overlap * 0.5);
                }
            }
        }
    }

    fn wall_collision(&mut self) {
        let map = &self.map;
        for e in &mut self.entities {
            if e.alive {
                clamp_to_map(map, &mut e.position, e.radius);
            }
        }
    }

    fn execute_attacks(&mut self) {
        let hero_id = self.hero_id;

        for i in 0..self.entities.len() {
            if !self.entities[i].alive || !self.entities[i].combat.ready() {
                continue;
            }
            let attacker_pos = self.entities[i].position;
            let attacker_radius = self.entities[i].radius;
            let attack_range = self.entities[i].combat.attack_range;
            let target_id = match self.entities[i].combat.target {
                Some(id) => id,
                None => continue,
            };

            let (target_pos, target_radius) =
                match self.entities.iter().find(|e| e.id == target_id && e.alive) {
                    Some(t) => (t.position, t.radius),
                    None => {
                        self.entities[i].combat.target = None;
                        continue;
                    }
                };

            let dist = attacker_pos.distance_to(target_pos);
            let reach = attacker_radius + target_radius + attack_range;
            if dist > reach {
                continue;
            }

            if !line_of_sight(&self.map, attacker_pos, target_pos) {
                continue;
            }

            let dmg_min = self.entities[i].combat.attack_damage_min;
            let dmg_max = self.entities[i].combat.attack_damage_max;
            let dmg = self.rng.gen_range(dmg_min..=dmg_max);
            self.entities[i].combat.start_cooldown();

            let target_idx = self
                .entities
                .iter()
                .position(|e| e.id == target_id)
                .unwrap();
            self.entities[target_idx].take_damage(dmg);

            if !self.entities[target_idx].alive && self.entities[target_idx].kind == EntityKind::Rat
            {
                let drop_pos = self.entities[target_idx].position;
                let item_id = self.alloc_item_id();
                self.items.push(GroundItem {
                    id: item_id,
                    kind: ItemKind::RatTail,
                    count: 1,
                    position: drop_pos,
                });
            }

            if !self.entities[target_idx].alive && target_id == hero_id {
                self.state = GameState::GameOver;
            }
        }
    }

    fn check_pickup(&mut self) {
        let hero_pos = self.hero().position;
        let mut to_remove = Vec::new();
        for item in &self.items {
            if hero_pos.distance_to(item.position) <= PICKUP_RADIUS {
                self.inventory.add(item.kind, item.count);
                to_remove.push(item.id);
            }
        }
        self.items.retain(|i| !to_remove.contains(&i.id));
    }

    fn check_game_state(&mut self) {
        if matches!(self.state, GameState::GameOver | GameState::Victory) {
            return;
        }
        let all_rats_dead = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Rat)
            .all(|e| !e.alive);
        if all_rats_dead {
            self.state = GameState::Victory;
        }
    }

    fn hero(&self) -> &Entity {
        self.entities.iter().find(|e| e.id == self.hero_id).unwrap()
    }

    fn hero_mut(&mut self) -> &mut Entity {
        let id = self.hero_id;
        self.entities.iter_mut().find(|e| e.id == id).unwrap()
    }

    pub fn make_snapshot(&self) -> RenderSnapshot {
        let tiles: Vec<TileRenderData> = (0..self.map.height as i32)
            .flat_map(|y| (0..self.map.width as i32).map(move |x| (x, y)))
            .map(|(x, y)| TileRenderData {
                x,
                y,
                kind: self.map.tile_kind(x, y),
            })
            .collect();

        let entities: Vec<EntityRenderData> = self
            .entities
            .iter()
            .map(|e| EntityRenderData {
                id: e.id,
                kind: e.kind,
                position: e.position,
                hp: e.hp,
                max_hp: e.max_hp,
                alive: e.alive,
            })
            .collect();

        let items: Vec<ItemRenderData> = self
            .items
            .iter()
            .map(|i| ItemRenderData {
                id: i.id,
                kind: i.kind,
                position: i.position,
            })
            .collect();

        let hero = self.hero();

        RenderSnapshot {
            frame_id: self.frame_id,
            sim_time: self.sim_time,
            paused: matches!(self.state, GameState::Paused),
            map_width: self.map.width,
            map_height: self.map.height,
            tiles,
            dirty_chunks: Vec::new(),
            entities,
            items,
            hero_status: HeroStatus {
                hp: hero.hp,
                max_hp: hero.max_hp,
                weapon_name: "Dagger",
                position: hero.position,
            },
            inventory: InventoryView {
                rat_tails: self.inventory.count(ItemKind::RatTail),
                show_panel: self.show_inventory,
            },
            inspect_info: self.inspect_info.clone(),
            game_state: self.state,
            rng_seed: self.rng_seed,
        }
    }
}

fn clamp_to_map(map: &TileMap, pos: &mut Vec2, radius: f32) {
    let tc = world_to_tile(*pos);
    let check_range = 2;
    for dy in -check_range..=check_range {
        for dx in -check_range..=check_range {
            let wx = tc.x + dx;
            let wy = tc.y + dy;
            if map.tile_kind(wx, wy) == TileKind::Wall {
                let tile_min_x = wx as f32;
                let tile_max_x = wx as f32 + 1.0;
                let tile_min_y = wy as f32;
                let tile_max_y = wy as f32 + 1.0;

                let nearest_x = pos.x.clamp(tile_min_x, tile_max_x);
                let nearest_y = pos.y.clamp(tile_min_y, tile_max_y);
                let delta = *pos - Vec2::new(nearest_x, nearest_y);
                let dist = delta.length();
                if dist < radius && dist > 1e-6 {
                    let normal = delta.normalized();
                    *pos = Vec2::new(nearest_x, nearest_y) + normal * radius;
                } else if dist < 1e-6 {
                    let tile_cx = wx as f32 + 0.5;
                    let tile_cy = wy as f32 + 0.5;
                    let away = (*pos - Vec2::new(tile_cx, tile_cy)).normalized();
                    *pos += away * (radius + 0.01);
                }
            }
        }
    }
}

fn line_of_sight(map: &TileMap, a: Vec2, b: Vec2) -> bool {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let steps = (dx.abs().max(dy.abs()) * 2.0).ceil() as i32 + 1;
    if steps <= 0 {
        return true;
    }
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (a.x + dx * t) as i32;
        let y = (a.y + dy * t) as i32;
        if map.tile_kind(x, y) == TileKind::Wall {
            return false;
        }
    }
    true
}
