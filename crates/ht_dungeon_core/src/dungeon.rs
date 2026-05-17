use crate::snapshot::{ChunkCoord, CHUNK_SIZE};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

pub const MAP_WIDTH: u32 = 80;
pub const MAP_HEIGHT: u32 = 60;
pub const ROOM_COUNT: usize = 10;
pub const STANDARD_DUNGEON_MAP: &str = include_str!("../../../data/maps/standard_dungeon.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileKind {
    Wall,
    Floor,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub kind: TileKind,
    pub hp: Option<i32>,
    pub revision: u32,
}

impl Tile {
    fn wall() -> Self {
        Self {
            kind: TileKind::Wall,
            hp: Some(10),
            revision: 0,
        }
    }

    fn floor() -> Self {
        Self {
            kind: TileKind::Floor,
            hp: None,
            revision: 0,
        }
    }
}

pub struct TileMap {
    pub width: u32,
    pub height: u32,
    tiles: Vec<Tile>,
    pub dirty_chunks: Vec<ChunkCoord>,
}

impl TileMap {
    pub fn new(width: u32, height: u32) -> Self {
        let tiles = vec![Tile::wall(); (width * height) as usize];
        Self {
            width,
            height,
            tiles,
            dirty_chunks: Vec::new(),
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            Some((y as u32 * self.width + x as u32) as usize)
        } else {
            None
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Option<&Tile> {
        self.index(x, y).map(|i| &self.tiles[i])
    }

    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        self.index(x, y).map(|i| &mut self.tiles[i])
    }

    pub fn set_floor(&mut self, x: i32, y: i32) {
        if let Some(i) = self.index(x, y) {
            if self.tiles[i].kind != TileKind::Floor {
                self.tiles[i] = Tile::floor();
                self.tiles[i].revision += 1;
                let chunk = ChunkCoord {
                    cx: x / CHUNK_SIZE,
                    cy: y / CHUNK_SIZE,
                };
                if !self.dirty_chunks.contains(&chunk) {
                    self.dirty_chunks.push(chunk);
                }
            }
        }
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.get(x, y).map_or(false, |t| t.kind == TileKind::Floor)
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    pub fn tile_kind(&self, x: i32, y: i32) -> TileKind {
        self.get(x, y).map_or(TileKind::Wall, |t| t.kind)
    }

    pub fn take_dirty_chunks(&mut self) -> Vec<ChunkCoord> {
        std::mem::take(&mut self.dirty_chunks)
    }
}

#[derive(Clone, Debug)]
pub struct Room {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Room {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    pub fn center_f(&self) -> (f32, f32) {
        let (cx, cy) = self.center();
        (cx as f32 + 0.5, cy as f32 + 0.5)
    }

    pub fn overlaps_with_padding(&self, other: &Room) -> bool {
        let pad = 1;
        !(self.x + self.width + pad <= other.x
            || other.x + other.width + pad <= self.x
            || self.y + self.height + pad <= other.y
            || other.y + other.height + pad <= self.y)
    }

    pub fn contains_tile(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub fn random_interior_point(&self, rng: &mut ChaCha8Rng) -> (i32, i32) {
        let margin = 1;
        let rx = rng
            .gen_range((self.x + margin)..(self.x + self.width - margin).max(self.x + margin + 1));
        let ry = rng
            .gen_range((self.y + margin)..(self.y + self.height - margin).max(self.y + margin + 1));
        (rx, ry)
    }
}

pub struct Dungeon {
    pub map: TileMap,
    pub rooms: Vec<Room>,
    pub spawns: Vec<DungeonSpawn>,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DungeonSpawnKind {
    Hero,
    Rat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DungeonSpawn {
    pub kind: DungeonSpawnKind,
    pub x: i32,
    pub y: i32,
}

impl DungeonSpawn {
    pub fn center_f(&self) -> (f32, f32) {
        (self.x as f32 + 0.5, self.y as f32 + 0.5)
    }
}

pub fn load_standard_dungeon() -> Dungeon {
    parse_dungeon_map(STANDARD_DUNGEON_MAP, 0)
}

fn parse_dungeon_map(source: &str, seed: u64) -> Dungeon {
    let rows: Vec<&str> = source
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with(';')
        })
        .collect();

    assert!(!rows.is_empty(), "dungeon map must contain a grid");

    let width = rows[0].chars().count();
    assert!(width > 0, "dungeon map must be at least one tile wide");
    for row in &rows {
        assert_eq!(
            row.chars().count(),
            width,
            "dungeon map rows must all have the same width"
        );
    }

    let mut map = TileMap::new(width as u32, rows.len() as u32);
    let mut spawns = Vec::new();

    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let x = x as i32;
            let y = y as i32;
            match ch {
                '#' => {}
                '.' => map.set_floor(x, y),
                '@' => {
                    map.set_floor(x, y);
                    spawns.push(DungeonSpawn {
                        kind: DungeonSpawnKind::Hero,
                        x,
                        y,
                    });
                }
                'r' => {
                    map.set_floor(x, y);
                    spawns.push(DungeonSpawn {
                        kind: DungeonSpawnKind::Rat,
                        x,
                        y,
                    });
                }
                other => panic!("unsupported dungeon map character: {other:?}"),
            }
        }
    }

    let hero_count = spawns
        .iter()
        .filter(|spawn| spawn.kind == DungeonSpawnKind::Hero)
        .count();
    assert_eq!(hero_count, 1, "dungeon map must contain exactly one hero");

    map.dirty_chunks.clear();

    Dungeon {
        map,
        rooms: Vec::new(),
        spawns,
        seed,
    }
}

pub fn generate_dungeon(seed: u64) -> Dungeon {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    loop {
        if let Some(d) = try_generate(&mut rng, seed) {
            return d;
        }
    }
}

fn try_generate(rng: &mut ChaCha8Rng, seed: u64) -> Option<Dungeon> {
    let mut map = TileMap::new(MAP_WIDTH, MAP_HEIGHT);
    let mut rooms: Vec<Room> = Vec::new();

    let max_attempts = 500;
    let mut attempts = 0;
    while rooms.len() < ROOM_COUNT && attempts < max_attempts {
        attempts += 1;
        let w = rng.gen_range(5..=12);
        let h = rng.gen_range(5..=10);
        let x = rng.gen_range(1..(MAP_WIDTH as i32 - w - 1));
        let y = rng.gen_range(1..(MAP_HEIGHT as i32 - h - 1));
        let room = Room {
            id: rooms.len(),
            x,
            y,
            width: w,
            height: h,
        };
        if rooms.iter().any(|r| r.overlaps_with_padding(&room)) {
            continue;
        }
        for ry in room.y..(room.y + room.height) {
            for rx in room.x..(room.x + room.width) {
                map.set_floor(rx, ry);
            }
        }
        rooms.push(room);
    }

    if rooms.len() < ROOM_COUNT {
        return None;
    }

    for i in 0..(rooms.len() - 1) {
        let (ax, ay) = rooms[i].center();
        let (bx, by) = rooms[i + 1].center();
        let h_first = rng.gen_bool(0.5);
        carve_corridor(&mut map, ax, ay, bx, by, h_first);
    }

    if !all_rooms_connected(&map, &rooms) {
        return None;
    }

    map.dirty_chunks.clear();

    Some(Dungeon {
        map,
        rooms,
        spawns: Vec::new(),
        seed,
    })
}

fn carve_corridor(map: &mut TileMap, ax: i32, ay: i32, bx: i32, by: i32, h_first: bool) {
    if h_first {
        let (x0, x1) = if ax <= bx { (ax, bx) } else { (bx, ax) };
        for x in x0..=x1 {
            map.set_floor(x, ay);
        }
        let (y0, y1) = if ay <= by { (ay, by) } else { (by, ay) };
        for y in y0..=y1 {
            map.set_floor(bx, y);
        }
    } else {
        let (y0, y1) = if ay <= by { (ay, by) } else { (by, ay) };
        for y in y0..=y1 {
            map.set_floor(ax, y);
        }
        let (x0, x1) = if ax <= bx { (ax, bx) } else { (bx, ax) };
        for x in x0..=x1 {
            map.set_floor(x, by);
        }
    }
}

fn all_rooms_connected(map: &TileMap, rooms: &[Room]) -> bool {
    if rooms.is_empty() {
        return true;
    }
    let start = rooms[0].center();
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![start];
    while let Some((x, y)) = stack.pop() {
        if !visited.insert((x, y)) {
            continue;
        }
        for (dx, dy) in [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)] {
            let nx = x + dx;
            let ny = y + dy;
            if map.is_walkable(nx, ny) && !visited.contains(&(nx, ny)) {
                stack.push((nx, ny));
            }
        }
    }
    rooms.iter().all(|r| {
        let (cx, cy) = r.center();
        visited.contains(&(cx, cy))
    })
}
