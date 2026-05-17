use bytemuck::{Pod, Zeroable};
use egui_wgpu::ScreenDescriptor;
use std::sync::Arc;
use wgpu::util::DeviceExt;

use ht_dungeon_core::config::AttackAnimationKeyframe;
use ht_dungeon_core::dungeon::TileKind;
use ht_dungeon_core::entity::EntityKind;
use ht_dungeon_core::snapshot::{AttackAnimationRenderData, RenderSnapshot};

use crate::camera::Camera;

const SHADER_SRC: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) local_pos: vec2<f32>,
    @location(1) world_pos: vec2<f32>,
    @location(2) size:      f32,
    @location(3) pad:       f32,
    @location(4) color:     vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let world = vec4<f32>(
        world_pos.x + local_pos.x * size,
        world_pos.y + local_pos.y * size,
        0.0,
        1.0,
    );
    out.clip_pos = camera.view_proj * world;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const SPRITE_SHADER_SRC: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var sprite_texture: texture_2d<f32>;

@group(1) @binding(1)
var sprite_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) local_pos: vec2<f32>,
    @location(1) uv:        vec2<f32>,
    @location(2) world_pos: vec2<f32>,
    @location(3) size:      vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let world = vec4<f32>(
        world_pos.x + local_pos.x * size.x,
        world_pos.y + local_pos.y * size.y,
        0.0,
        1.0,
    );
    out.clip_pos = camera.view_proj * world;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.uv);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct QuadInstance {
    pub world_pos: [f32; 2],
    pub size: f32,
    pub _pad: f32,
    pub color: [f32; 4],
}

impl QuadInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SpriteVertex {
    local_pos: [f32; 2],
    uv: [f32; 2],
}

impl SpriteVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SpriteInstance {
    world_pos: [f32; 2],
    size: [f32; 2],
}

impl SpriteInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const QUAD_VERTS: &[[f32; 2]] = &[
    [-0.5, -0.5],
    [0.5, -0.5],
    [0.5, 0.5],
    [-0.5, -0.5],
    [0.5, 0.5],
    [-0.5, 0.5],
];

const HERO_SPRITE_WIDTH: f32 = 42.0;
const HERO_SPRITE_HEIGHT: f32 = 51.0;
const HERO_WORLD_HEIGHT: f32 = 1.35;
const HERO_WORLD_WIDTH: f32 = HERO_WORLD_HEIGHT * HERO_SPRITE_WIDTH / HERO_SPRITE_HEIGHT;

// Keep the circle/gameplay position two source pixels left of the visual leg center.
const HERO_LEG_ANCHOR_X: f32 = 17.0 / HERO_SPRITE_WIDTH;
const HERO_LEG_ANCHOR_Y: f32 = 44.0 / HERO_SPRITE_HEIGHT;

const HERO_SPRITE_VERTS: &[SpriteVertex] = &[
    SpriteVertex {
        local_pos: [-HERO_LEG_ANCHOR_X, -HERO_LEG_ANCHOR_Y],
        uv: [0.0, 0.0],
    },
    SpriteVertex {
        local_pos: [1.0 - HERO_LEG_ANCHOR_X, -HERO_LEG_ANCHOR_Y],
        uv: [1.0, 0.0],
    },
    SpriteVertex {
        local_pos: [1.0 - HERO_LEG_ANCHOR_X, 1.0 - HERO_LEG_ANCHOR_Y],
        uv: [1.0, 1.0],
    },
    SpriteVertex {
        local_pos: [-HERO_LEG_ANCHOR_X, -HERO_LEG_ANCHOR_Y],
        uv: [0.0, 0.0],
    },
    SpriteVertex {
        local_pos: [1.0 - HERO_LEG_ANCHOR_X, 1.0 - HERO_LEG_ANCHOR_Y],
        uv: [1.0, 1.0],
    },
    SpriteVertex {
        local_pos: [-HERO_LEG_ANCHOR_X, 1.0 - HERO_LEG_ANCHOR_Y],
        uv: [0.0, 1.0],
    },
];

const HERO_MARKER_COLOR: [f32; 4] = [0.08, 0.28, 0.52, 0.82];
const HERO_MARKER_SIZE: f32 = 0.58;

const RAT_SPRITE_WIDTH: f32 = 28.0;
const RAT_SPRITE_HEIGHT: f32 = 17.0;
const RAT_WORLD_HEIGHT: f32 = 0.45;
const RAT_WORLD_WIDTH: f32 = RAT_WORLD_HEIGHT * RAT_SPRITE_WIDTH / RAT_SPRITE_HEIGHT;

const CENTERED_SPRITE_VERTS: &[SpriteVertex] = &[
    SpriteVertex {
        local_pos: [-0.5, -0.5],
        uv: [0.0, 0.0],
    },
    SpriteVertex {
        local_pos: [0.5, -0.5],
        uv: [1.0, 0.0],
    },
    SpriteVertex {
        local_pos: [0.5, 0.5],
        uv: [1.0, 1.0],
    },
    SpriteVertex {
        local_pos: [-0.5, -0.5],
        uv: [0.0, 0.0],
    },
    SpriteVertex {
        local_pos: [0.5, 0.5],
        uv: [1.0, 1.0],
    },
    SpriteVertex {
        local_pos: [-0.5, 0.5],
        uv: [0.0, 1.0],
    },
];

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,

    pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    quad_vertex_buf: wgpu::Buffer,
    circle_vertex_buf: wgpu::Buffer,
    circle_vertex_count: u32,
    hero_sprite_vertex_buf: wgpu::Buffer,
    rat_sprite_vertex_buf: wgpu::Buffer,
    quad_instance_buf: wgpu::Buffer,
    quad_instance_cap: usize,
    circle_instance_buf: wgpu::Buffer,
    circle_instance_cap: usize,
    hero_sprite_instance_buf: wgpu::Buffer,
    hero_sprite_instance_cap: usize,
    rat_sprite_instance_buf: wgpu::Buffer,
    rat_sprite_instance_cap: usize,
    camera_buf: wgpu::Buffer,
    camera_bg: wgpu::BindGroup,
    hero_texture_bg: wgpu::BindGroup,
    rat_texture_bg: wgpu::BindGroup,

    pub egui_renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();

        // Prefer Vulkan: wgpu 22's DX12 backend omits the RENDER_TARGET→PRESENT
        // barrier before Present, causing D3D12 validation errors at runtime.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("No suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("Failed to create wgpu device");

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });
        let sprite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_shader"),
            source: wgpu::ShaderSource::Wgsl(SPRITE_SHADER_SRC.into()),
        });

        let camera_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buf"),
            contents: bytemuck::cast_slice(&[0f32; 16]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let hero_texture_bg = create_texture_bind_group(
            &device,
            &queue,
            &texture_bgl,
            "hero",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/hero.png"
            )),
            "assets/hero.png",
        );
        let rat_texture_bg = create_texture_bind_group(
            &device,
            &queue,
            &texture_bgl,
            "rat",
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/rat.png")),
            "assets/rat.png",
        );

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&camera_bgl],
            push_constant_ranges: &[],
        });
        let sprite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sprite_pipeline_layout"),
                bind_group_layouts: &[&camera_bgl, &texture_bgl],
                push_constant_ranges: &[],
            });

        let quad_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad_verts"),
            contents: bytemuck::cast_slice(QUAD_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let circle_verts = build_circle_vertices(32);
        let circle_vertex_count = circle_verts.len() as u32;
        let circle_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("circle_verts"),
            contents: bytemuck::cast_slice(&circle_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let hero_sprite_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hero_sprite_verts"),
            contents: bytemuck::cast_slice(HERO_SPRITE_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let rat_sprite_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rat_sprite_verts"),
            contents: bytemuck::cast_slice(CENTERED_SPRITE_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let quad_instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad_instance_buf"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let circle_instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("circle_instance_buf"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hero_sprite_instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hero_sprite_instance_buf"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rat_sprite_instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rat_sprite_instance_buf"),
            size: 0,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: (std::mem::size_of::<f32>() * 2) as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        }],
                    },
                    QuadInstance::desc(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite_pipeline"),
            layout: Some(&sprite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sprite_shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[SpriteVertex::desc(), SpriteInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sprite_shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            sprite_pipeline,
            quad_vertex_buf,
            circle_vertex_buf,
            circle_vertex_count,
            hero_sprite_vertex_buf,
            rat_sprite_vertex_buf,
            quad_instance_buf,
            quad_instance_cap: 0,
            circle_instance_buf,
            circle_instance_cap: 0,
            hero_sprite_instance_buf,
            hero_sprite_instance_cap: 0,
            rat_sprite_instance_buf,
            rat_sprite_instance_cap: 0,
            camera_buf,
            camera_bg,
            hero_texture_bg,
            rat_texture_bg,
            egui_renderer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(
        &mut self,
        snapshot: &RenderSnapshot,
        camera: &Camera,
        show_hero_circle: bool,
        egui_ctx: &egui::Context,
        egui_output: egui::FullOutput,
        pixels_per_point: f32,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = self.size.width as f32 / self.size.height as f32;
        let vp: [[f32; 4]; 4] = camera.view_proj(aspect);
        let vp_flat: [f32; 16] = unsafe { std::mem::transmute(vp) };
        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::cast_slice(&vp_flat));

        let batches = build_batches(snapshot, show_hero_circle);
        self.upload_batches(&batches);

        // Egui texture updates and tessellation
        for (id, delta) in &egui_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point,
        };
        let paint_jobs = egui_ctx.tessellate(egui_output.shapes, pixels_per_point);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Single pass: clear → world quads → egui overlay.
        // Using two separate passes on the same surface texture confuses the D3D12
        // backend's RENDER_TARGET→PRESENT barrier insertion, so we do everything here.
        {
            let color_attach = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.04,
                        b: 0.04,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let pass_desc = wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &color_attach,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            let mut pass = encoder.begin_render_pass(&pass_desc);

            if !batches.quads.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_vertex_buffer(0, self.quad_vertex_buf.slice(..));
                pass.set_vertex_buffer(1, self.quad_instance_buf.slice(..));
                pass.draw(0..6, 0..batches.quads.len() as u32);
            }

            if !batches.circles.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_vertex_buffer(0, self.circle_vertex_buf.slice(..));
                pass.set_vertex_buffer(1, self.circle_instance_buf.slice(..));
                pass.draw(0..self.circle_vertex_count, 0..batches.circles.len() as u32);
            }

            if !batches.hero_sprites.is_empty() {
                pass.set_pipeline(&self.sprite_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.hero_texture_bg, &[]);
                pass.set_vertex_buffer(0, self.hero_sprite_vertex_buf.slice(..));
                pass.set_vertex_buffer(1, self.hero_sprite_instance_buf.slice(..));
                pass.draw(0..6, 0..batches.hero_sprites.len() as u32);
            }

            if !batches.rat_sprites.is_empty() {
                pass.set_pipeline(&self.sprite_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.rat_texture_bg, &[]);
                pass.set_vertex_buffer(0, self.rat_sprite_vertex_buf.slice(..));
                pass.set_vertex_buffer(1, self.rat_sprite_instance_buf.slice(..));
                pass.draw(0..6, 0..batches.rat_sprites.len() as u32);
            }

            // egui_wgpu 0.29 requires RenderPass<'static>; raw-pointer cast is sound
            // because the pass is never stored and outlives this block.
            unsafe {
                let p: *mut wgpu::RenderPass<'static> =
                    &mut pass as *mut wgpu::RenderPass<'_> as *mut wgpu::RenderPass<'static>;
                self.egui_renderer
                    .render(&mut *p, &paint_jobs, &screen_descriptor);
            }
        }

        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn upload_batches(&mut self, batches: &RenderBatches) {
        upload_instances(
            &self.device,
            &self.queue,
            &mut self.quad_instance_buf,
            &mut self.quad_instance_cap,
            "quad_instance_buf",
            &batches.quads,
        );
        upload_instances(
            &self.device,
            &self.queue,
            &mut self.circle_instance_buf,
            &mut self.circle_instance_cap,
            "circle_instance_buf",
            &batches.circles,
        );
        upload_instances(
            &self.device,
            &self.queue,
            &mut self.hero_sprite_instance_buf,
            &mut self.hero_sprite_instance_cap,
            "hero_sprite_instance_buf",
            &batches.hero_sprites,
        );
        upload_instances(
            &self.device,
            &self.queue,
            &mut self.rat_sprite_instance_buf,
            &mut self.rat_sprite_instance_cap,
            "rat_sprite_instance_buf",
            &batches.rat_sprites,
        );
    }
}

fn create_texture_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture_bgl: &wgpu::BindGroupLayout,
    label: &str,
    bytes: &[u8],
    asset_path: &str,
) -> wgpu::BindGroup {
    let image = image::load_from_memory(bytes)
        .unwrap_or_else(|_| panic!("{asset_path} must be a valid PNG"))
        .to_rgba8();
    let (width, height) = image.dimensions();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&format!("{label}_texture")),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &image,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(&format!("{label}_sampler")),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label}_texture_bg")),
        layout: texture_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    })
}

fn upload_instances<T: Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &mut wgpu::Buffer,
    capacity: &mut usize,
    label: &'static str,
    instances: &[T],
) {
    if instances.is_empty() {
        return;
    }
    let needed = instances.len();
    if needed > *capacity {
        *buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        *capacity = needed;
    } else {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(instances));
    }
}

fn build_circle_vertices(segments: u32) -> Vec<[f32; 2]> {
    let mut verts = Vec::with_capacity((segments as usize) * 3);
    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        verts.push([0.0, 0.0]);
        verts.push([a0.cos() * 0.5, a0.sin() * 0.5]);
        verts.push([a1.cos() * 0.5, a1.sin() * 0.5]);
    }
    verts
}

struct RenderBatches {
    quads: Vec<QuadInstance>,
    circles: Vec<QuadInstance>,
    hero_sprites: Vec<SpriteInstance>,
    rat_sprites: Vec<SpriteInstance>,
}

fn build_batches(snapshot: &RenderSnapshot, show_hero_circle: bool) -> RenderBatches {
    let mut quads =
        Vec::with_capacity(snapshot.tiles.len() + snapshot.entities.len() + snapshot.items.len());
    let mut circles = Vec::with_capacity(1);
    let mut hero_sprites = Vec::with_capacity(1);
    let mut rat_sprites = Vec::new();

    for tile in &snapshot.tiles {
        let color = match tile.kind {
            TileKind::Floor => [0.22f32, 0.18, 0.15, 1.0],
            TileKind::Wall => [0.48f32, 0.46, 0.42, 1.0],
        };
        quads.push(QuadInstance {
            world_pos: [tile.x as f32 + 0.5, tile.y as f32 + 0.5],
            size: 1.0,
            _pad: 0.0,
            color,
        });
    }

    for item in &snapshot.items {
        quads.push(QuadInstance {
            world_pos: [item.position.x, item.position.y],
            size: 0.30,
            _pad: 0.0,
            color: [0.85, 0.75, 0.20, 1.0],
        });
    }

    for e in &snapshot.entities {
        if !e.alive {
            continue;
        }
        match e.kind {
            EntityKind::Hero => {
                let attack_offset = attack_animation_offset(
                    e.kind,
                    e.attack_animation.as_ref(),
                    snapshot
                        .config
                        .hero
                        .attack_animation_keyframes
                        .as_deref()
                        .unwrap_or(&snapshot.config.standard_attack_animation.keyframes),
                );
                if show_hero_circle {
                    circles.push(QuadInstance {
                        world_pos: [e.position.x, e.position.y],
                        size: HERO_MARKER_SIZE,
                        _pad: 0.0,
                        color: HERO_MARKER_COLOR,
                    });
                }
                hero_sprites.push(SpriteInstance {
                    world_pos: [
                        e.position.x + attack_offset[0],
                        e.position.y + attack_offset[1],
                    ],
                    size: [HERO_WORLD_WIDTH, HERO_WORLD_HEIGHT],
                });
            }
            EntityKind::Rat => {
                let attack_offset = attack_animation_offset(
                    e.kind,
                    e.attack_animation.as_ref(),
                    snapshot
                        .config
                        .rat
                        .actor
                        .attack_animation_keyframes
                        .as_deref()
                        .unwrap_or(&snapshot.config.standard_attack_animation.keyframes),
                );
                rat_sprites.push(SpriteInstance {
                    world_pos: [
                        e.position.x + attack_offset[0],
                        e.position.y + attack_offset[1],
                    ],
                    size: [RAT_WORLD_WIDTH, RAT_WORLD_HEIGHT],
                });
            }
        }
    }

    RenderBatches {
        quads,
        circles,
        hero_sprites,
        rat_sprites,
    }
}

fn attack_animation_offset(
    kind: EntityKind,
    animation: Option<&AttackAnimationRenderData>,
    keyframes: &[AttackAnimationKeyframe],
) -> [f32; 2] {
    let Some(animation) = animation else {
        return [0.0, 0.0];
    };

    let pixels = sample_timeline(animation.elapsed, keyframes);
    let world_per_pixel = match kind {
        EntityKind::Hero => HERO_WORLD_HEIGHT / HERO_SPRITE_HEIGHT,
        EntityKind::Rat => RAT_WORLD_HEIGHT / RAT_SPRITE_HEIGHT,
    };

    [
        animation.direction.x * pixels * world_per_pixel,
        animation.direction.y * pixels * world_per_pixel,
    ]
}

fn sample_timeline(elapsed: f32, keys: &[AttackAnimationKeyframe]) -> f32 {
    if elapsed <= keys[0].time_seconds {
        return keys[0].offset_pixels;
    }

    for window in keys.windows(2) {
        let start = window[0];
        let end = window[1];
        let start_time = start.time_seconds;
        let end_time = end.time_seconds;
        if elapsed <= end_time {
            let t = ((elapsed - start_time) / (end_time - start_time)).clamp(0.0, 1.0);
            return start.offset_pixels + (end.offset_pixels - start.offset_pixels) * t;
        }
    }

    keys[keys.len() - 1].offset_pixels
}
