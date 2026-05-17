pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Camera {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y, zoom: 20.0 }
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 0.9).max(4.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom * 1.1).min(80.0);
    }

    pub fn zoom_scroll(&mut self, delta: f32) {
        if delta > 0.0 {
            self.zoom_in();
        } else if delta < 0.0 {
            self.zoom_out();
        }
    }

    /// Half-height of visible world area.
    pub fn half_h(&self) -> f32 {
        self.zoom * 0.5
    }

    pub fn half_w(&self, aspect: f32) -> f32 {
        self.half_h() * aspect
    }

    /// Build the 4×4 column-major orthographic matrix for the GPU uniform.
    /// World Y increases downward; NDC Y increases upward.
    pub fn view_proj(&self, aspect: f32) -> [[f32; 4]; 4] {
        let hw = self.half_w(aspect);
        let hh = self.half_h();
        let cx = self.x;
        let cy = self.y;
        // Column-major (WGSL mat4x4): each inner array is one column [row0..row3].
        [
            [1.0 / hw, 0.0, 0.0, 0.0],
            [0.0, -1.0 / hh, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-cx / hw, cy / hh, 0.0, 1.0],
        ]
    }

    /// Convert screen pixel position to world coordinates.
    pub fn screen_to_world(&self, px: f32, py: f32, screen_w: f32, screen_h: f32) -> (f32, f32) {
        let aspect = screen_w / screen_h;
        let hw = self.half_w(aspect);
        let hh = self.half_h();
        let wx = self.x + (px / screen_w - 0.5) * 2.0 * hw;
        let wy = self.y + (py / screen_h - 0.5) * 2.0 * hh;
        (wx, wy)
    }
}
