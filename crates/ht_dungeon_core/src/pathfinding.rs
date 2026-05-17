use crate::dungeon::TileMap;
use crate::entity::{world_to_tile, Vec2};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

const ORTHO_COST: f32 = 1.0;
const DIAG_COST: f32 = 1.4142;

#[derive(PartialEq)]
struct Node {
    f: f32,
    g: f32,
    x: i32,
    y: i32,
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn octile(ax: i32, ay: i32, bx: i32, by: i32) -> f32 {
    let dx = (ax - bx).unsigned_abs() as f32;
    let dy = (ay - by).unsigned_abs() as f32;
    let (mn, mx) = if dx < dy { (dx, dy) } else { (dy, dx) };
    DIAG_COST * mn + ORTHO_COST * (mx - mn)
}

fn diagonal_ok(map: &TileMap, x: i32, y: i32, dx: i32, dy: i32) -> bool {
    map.is_walkable(x + dx, y) && map.is_walkable(x, y + dy)
}

pub fn find_path(map: &TileMap, start: Vec2, goal: Vec2) -> Option<Vec<Vec2>> {
    let sc = world_to_tile(start);
    let gc = world_to_tile(goal);

    if !map.is_walkable(sc.x, sc.y) || !map.is_walkable(gc.x, gc.y) {
        return None;
    }

    if sc.x == gc.x && sc.y == gc.y {
        return Some(vec![goal]);
    }

    let mut open: BinaryHeap<Node> = BinaryHeap::new();
    let mut g_map: HashMap<(i32, i32), f32> = HashMap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();

    let h0 = octile(sc.x, sc.y, gc.x, gc.y);
    open.push(Node {
        f: h0,
        g: 0.0,
        x: sc.x,
        y: sc.y,
    });
    g_map.insert((sc.x, sc.y), 0.0);

    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    while let Some(Node { g, x, y, .. }) = open.pop() {
        if x == gc.x && y == gc.y {
            let path = reconstruct(came_from, (x, y), goal);
            return Some(path);
        }

        let best_g = g_map.get(&(x, y)).copied().unwrap_or(f32::MAX);
        if g > best_g + 1e-5 {
            continue;
        }

        for &(dx, dy) in &DIRS {
            let nx = x + dx;
            let ny = y + dy;
            if !map.is_walkable(nx, ny) {
                continue;
            }
            let is_diag = dx != 0 && dy != 0;
            if is_diag && !diagonal_ok(map, x, y, dx, dy) {
                continue;
            }
            let step = if is_diag { DIAG_COST } else { ORTHO_COST };
            let ng = g + step;
            let prev_g = g_map.get(&(nx, ny)).copied().unwrap_or(f32::MAX);
            if ng < prev_g - 1e-5 {
                g_map.insert((nx, ny), ng);
                came_from.insert((nx, ny), (x, y));
                let h = octile(nx, ny, gc.x, gc.y);
                open.push(Node {
                    f: ng + h,
                    g: ng,
                    x: nx,
                    y: ny,
                });
            }
        }
    }

    None
}

fn reconstruct(
    came_from: HashMap<(i32, i32), (i32, i32)>,
    end: (i32, i32),
    goal: Vec2,
) -> Vec<Vec2> {
    let mut path = Vec::new();
    let mut cur = end;
    while let Some(&prev) = came_from.get(&cur) {
        path.push(Vec2::new(cur.0 as f32 + 0.5, cur.1 as f32 + 0.5));
        cur = prev;
    }
    path.reverse();
    if let Some(last) = path.last_mut() {
        *last = goal;
    } else {
        path.push(goal);
    }
    path
}

pub fn nearest_walkable(map: &TileMap, target: Vec2) -> Option<Vec2> {
    let tc = world_to_tile(target);
    if map.is_walkable(tc.x, tc.y) {
        return Some(target);
    }
    for radius in 1..=5i32 {
        let mut best: Option<Vec2> = None;
        let mut best_dist = f32::MAX;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let nx = tc.x + dx;
                let ny = tc.y + dy;
                if map.is_walkable(nx, ny) {
                    let p = Vec2::new(nx as f32 + 0.5, ny as f32 + 0.5);
                    let d = p.distance_to(target);
                    if d < best_dist {
                        best_dist = d;
                        best = Some(p);
                    }
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}
