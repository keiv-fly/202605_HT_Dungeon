# HT Dungeon MVP Milestones

Source spec: `20260513_specs.md`

## MVP Goal

Deliver a first playable version of HT Dungeon where one hero explores one generated dungeon floor, fights rats, collects rat tails, uses pause, and reaches game over or victory.

The MVP is complete when the game supports:

1. Starting a new game.
2. Generating one connected 10-room dungeon floor.
3. Spawning one hero and rats according to room rules.
4. Rendering walls, floors, hero, rats, and rat tail drops.
5. Mouse-driven movement, combat, pickup, and inspect.
6. Real-time-with-pause simulation.
7. Rat AI, combat, death, loot, inventory, game over, and victory.

---

## Milestone 1: Project Skeleton and Thread Boundary

### Deliverables

* Create Rust workspace layout with `ht_dungeon_core` and `ht_dungeon_app`.
* Keep all game logic in `ht_dungeon_core`.
* Keep windowing, rendering, input, camera, and UI in `ht_dungeon_app`.
* Add fixed-step logic thread at 60 ticks per second.
* Add bounded crossbeam channels:
  * `PlayerCommand` from render/UI thread to logic thread.
  * `RenderSnapshot` from logic thread to render/UI thread.
* Define initial shared types for commands, snapshots, game state, entity ids, item ids, tile coordinates, world coordinates, and inventory views.

### Exit Criteria

* App starts on Windows x86_64.
* Logic thread runs independently from render/UI thread.
* Renderer can receive and retain the newest snapshot.
* Core crate has no dependency on `wgpu`, `winit`, `egui`, or windowing APIs.

---

## Milestone 2: Core World Model and Deterministic Dungeon Generation

### Deliverables

* Implement `GameWorld`, `TileMap`, `Tile`, `TileKind`, `Room`, and seed handling.
* Generate an 80 x 60 tile dungeon from a `u64` seed.
* Place exactly 10 non-overlapping rooms with at least 1 tile of padding.
* Carve L-shaped corridors between rooms in placement order.
* Verify all rooms are reachable from room 1.
* Track tile revisions and dirty chunk coordinates for 16 x 16 tile chunks.

### Exit Criteria

* Fixed seeds produce reproducible dungeons.
* Exactly 10 rooms are generated.
* Rooms do not overlap.
* Room sizes vary.
* All rooms are connected by floor tiles.
* Dirty chunk data exists in snapshots even if no runtime terrain mutation is exposed yet.

---

## Milestone 3: Entities, Spawning, and Inventory Data

### Deliverables

* Implement hero and rat entity data.
* Spawn the hero in the starting room.
* Spawn rats by room rules:
  * Room 1: 1 rat.
  * Room 2: 1 rat.
  * Rooms 3-10: 1 or 2 rats each.
* Enforce rat placement on floor tiles with no entity overlap.
* Add stackable rat tail inventory model.
* Add ground item model for rat tail drops.
* Include entities, items, hero status, and inventory view in `RenderSnapshot`.

### Exit Criteria

* Total rat count is always between 10 and 18.
* Rats never spawn inside walls.
* Rats do not overlap each other or the hero on spawn.
* Snapshot data contains enough information for renderer and UI to display the world state.

---

## Milestone 4: Movement, Collision, and Pathfinding

### Deliverables

* Implement A* pathfinding over walkable floor tiles.
* Support 4-directional and valid 8-directional movement.
* Prevent diagonal corner cutting.
* Move actors continuously through world coordinates along path waypoints.
* Add circular wall collision for hero and rats.
* Add soft actor separation.
* Implement hero movement command from clicked floor or closest reachable floor near a clicked wall.

### Exit Criteria

* Hero moves to clicked floor positions.
* Hero positions are continuous and not locked to tile centers.
* Hero and rats cannot walk through walls.
* Hero and rats cannot pass through blocked diagonal corners.
* Actors separate cleanly instead of occupying the same position.

---

## Milestone 5: Combat, Rat AI, Loot, and Victory State

### Deliverables

* Implement rat sight checks using distance and wall-blocked line of sight.
* Add rat AI states: idle, chasing hero, attacking hero, dead.
* Implement attack cooldowns.
* Implement hero dagger attack for 1-2 damage.
* Implement rat attack for 1 damage.
* Use movement-to-attack stop distance and attack range hysteresis.
* Drop 1 rat tail when a rat dies.
* Automatically pick up rat tails within pickup radius.
* Set game over when hero HP reaches 0.
* Set victory when all rats are dead.

### Exit Criteria

* Clicking a rat makes the hero approach and attack it.
* Rats chase and attack the hero after detecting the hero.
* Rat deaths reliably spawn rat tail items.
* Picked-up rat tails stack in inventory.
* Game over and victory states are reachable.

---

## Milestone 6: Rendering, Camera, and MVP UI

### Deliverables

* Create a `wgpu` top-down 2D renderer.
* Render floor and wall tiles using chunked tile buffers.
* Rebuild only dirty chunks when tile revisions change.
* Render hero, rats, and rat tail item markers.
* Add orthographic camera with pan, zoom, and center-on-hero behavior.
* Add MVP `egui` panels over `wgpu`:
  * HP display.
  * Pause indicator.
  * Inventory panel.
  * Inspect panel.
  * Game over text.
  * Victory text.
  * Debug seed display.

### Exit Criteria

* Dungeon geometry renders from tiles, not from a static background.
* Hero, rats, and rat tails are visually distinguishable.
* Camera controls work while running and paused.
* UI displays current hero HP, inventory count, pause state, inspect details, and final game state.
* Dirty chunk redraw path can be exercised by changing a tile in debug/test code.

---

## Milestone 7: Input, Pause, and Inspect Flow

### Deliverables

* Implement keyboard controls:
  * `Space`: pause/unpause.
  * `WASD`: pan camera.
  * Mouse wheel, `Q`, `E`: zoom.
  * `C`: center camera on hero.
  * `I`: toggle inventory.
  * `Esc`: close panel or pause.
* Implement authoritative logic-side click handling.
* Left-click behavior:
  * Floor: move.
  * Enemy: attack.
  * Item: move to pickup.
  * Wall: move as close as possible or show blocked marker.
* Right-click inspect for floor, wall, hero, rat, and rat tail.
* Allow commands to be queued while paused and executed after unpause.

### Exit Criteria

* Pause stops movement, AI, combat, cooldowns, and pickup.
* Camera and UI remain responsive while paused.
* Inspect works while paused.
* Queued move and attack orders issued while paused execute after unpausing.
* Click behavior matches object type under the cursor.

---

## Milestone 8: MVP Acceptance and Polish Pass

### Deliverables

* Add focused automated tests for:
  * Dungeon generation.
  * Rat placement.
  * Movement constraints.
  * Combat outcomes.
  * Loot and inventory stacking.
  * Pause behavior.
* Add a manual smoke-test checklist for rendering and input.
* Tune values from the spec:
  * Hero HP 10, speed 3.0, radius 0.30.
  * Rat HP 1, speed 2.0, radius 0.25, sight range 7.
  * Hero melee range 0.75 and cooldown 0.7 seconds.
  * Rat melee range 0.65 and cooldown 1.0 second.
  * Pickup radius 0.5.
* Confirm Windows build and run path.

### Exit Criteria

* Acceptance tests from the spec pass or have documented manual coverage where automation is impractical.
* A new player can start the game, move, fight rats, collect tails, inspect objects, pause, and reach victory or game over.
* Known MVP limitations are documented separately from bugs.

---

## Suggested Build Order

1. Milestone 1: Project Skeleton and Thread Boundary.
2. Milestone 2: Core World Model and Deterministic Dungeon Generation.
3. Milestone 3: Entities, Spawning, and Inventory Data.
4. Milestone 6 partial: Minimal renderer for tiles and entities.
5. Milestone 4: Movement, Collision, and Pathfinding.
6. Milestone 5: Combat, Rat AI, Loot, and Victory State.
7. Milestone 7: Input, Pause, and Inspect Flow.
8. Milestone 6 complete: Camera, dirty chunks, and UI polish.
9. Milestone 8: Acceptance and polish pass.

This order gets visual feedback early while keeping game rules testable in the core crate.
