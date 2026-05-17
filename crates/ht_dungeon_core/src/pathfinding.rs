use crate::dungeon::{TileKind, TileMap};
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

pub fn find_path(map: &TileMap, start: Vec2, goal: Vec2, actor_radius: f32) -> Option<Vec<Vec2>> {
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
            return Some(smooth_path(map, start, path, actor_radius));
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

fn smooth_path(map: &TileMap, start: Vec2, path: Vec<Vec2>, actor_radius: f32) -> Vec<Vec2> {
    if path.len() <= 1 {
        return path;
    }

    let mut points = Vec::with_capacity(path.len() + 1);
    points.push(start);
    points.extend(path);

    let mut result = Vec::new();
    let mut current_index = 0;
    result.push(points[0]);

    while current_index < points.len() - 1 {
        let mut next_index = points.len() - 1;

        while next_index > current_index + 1 {
            if has_line_of_sight(map, points[current_index], points[next_index], actor_radius) {
                break;
            }
            next_index -= 1;
        }

        result.push(points[next_index]);
        current_index = next_index;
    }

    result.into_iter().skip(1).collect()
}

fn has_line_of_sight(map: &TileMap, a: Vec2, b: Vec2, actor_radius: f32) -> bool {
    let min_x = a.x.min(b.x) - actor_radius;
    let max_x = a.x.max(b.x) + actor_radius;
    let min_y = a.y.min(b.y) - actor_radius;
    let max_y = a.y.max(b.y) + actor_radius;

    let tile_min_x = min_x.floor() as i32;
    let tile_max_x = max_x.floor() as i32;
    let tile_min_y = min_y.floor() as i32;
    let tile_max_y = max_y.floor() as i32;

    for y in tile_min_y..=tile_max_y {
        for x in tile_min_x..=tile_max_x {
            if map.tile_kind(x, y) == TileKind::Wall {
                let rect = Rect::expanded_tile(x, y, actor_radius);
                if segment_intersects_rect(a, b, rect) {
                    return false;
                }
            }
        }
    }

    true
}

#[derive(Clone, Copy)]
struct Rect {
    min: Vec2,
    max: Vec2,
}

impl Rect {
    fn expanded_tile(x: i32, y: i32, radius: f32) -> Self {
        Self {
            min: Vec2::new(x as f32 - radius, y as f32 - radius),
            max: Vec2::new(x as f32 + 1.0 + radius, y as f32 + 1.0 + radius),
        }
    }
}

fn segment_intersects_rect(a: Vec2, b: Vec2, rect: Rect) -> bool {
    point_inside_rect(a, rect)
        || point_inside_rect(b, rect)
        || segment_intersects_segment(a, b, rect.min, Vec2::new(rect.max.x, rect.min.y))
        || segment_intersects_segment(a, b, Vec2::new(rect.max.x, rect.min.y), rect.max)
        || segment_intersects_segment(a, b, rect.max, Vec2::new(rect.min.x, rect.max.y))
        || segment_intersects_segment(a, b, Vec2::new(rect.min.x, rect.max.y), rect.min)
}

fn point_inside_rect(p: Vec2, rect: Rect) -> bool {
    p.x >= rect.min.x && p.x <= rect.max.x && p.y >= rect.min.y && p.y <= rect.max.y
}

fn segment_intersects_segment(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let ab = b - a;
    let ac = c - a;
    let ad = d - a;
    let cd = d - c;
    let ca = a - c;
    let cb = b - c;

    let d1 = cross(ab, ac);
    let d2 = cross(ab, ad);
    let d3 = cross(cd, ca);
    let d4 = cross(cd, cb);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }

    const EPS: f32 = 1e-6;
    (d1.abs() <= EPS && point_on_segment(c, a, b))
        || (d2.abs() <= EPS && point_on_segment(d, a, b))
        || (d3.abs() <= EPS && point_on_segment(a, c, d))
        || (d4.abs() <= EPS && point_on_segment(b, c, d))
}

fn point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> bool {
    const EPS: f32 = 1e-6;
    p.x >= a.x.min(b.x) - EPS
        && p.x <= a.x.max(b.x) + EPS
        && p.y >= a.y.min(b.y) - EPS
        && p.y <= a.y.max(b.y) + EPS
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
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

#[cfg(test)]
mod tests {
    use super::*;

    fn open_map(width: u32, height: u32) -> TileMap {
        let mut map = TileMap::new(width, height);
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                map.set_floor(x, y);
            }
        }
        map
    }

    #[test]
    fn smooth_path_removes_visible_intermediate_points() {
        let map = open_map(6, 6);
        let start = Vec2::new(1.5, 1.5);
        let goal = Vec2::new(4.5, 4.5);
        let raw_path = vec![
            Vec2::new(2.5, 1.5),
            Vec2::new(3.5, 2.5),
            Vec2::new(4.5, 3.5),
            goal,
        ];

        let smoothed = smooth_path(&map, start, raw_path, 0.30);

        assert_eq!(smoothed, vec![goal]);
    }

    #[test]
    fn line_of_sight_respects_actor_radius_near_walls() {
        let mut map = open_map(6, 6);
        if let Some(tile) = map.get_mut(2, 2) {
            tile.kind = TileKind::Wall;
        }

        let a = Vec2::new(1.5, 1.75);
        let b = Vec2::new(3.5, 1.75);

        assert!(has_line_of_sight(&map, a, b, 0.0));
        assert!(!has_line_of_sight(&map, a, b, 0.30));
    }
}
