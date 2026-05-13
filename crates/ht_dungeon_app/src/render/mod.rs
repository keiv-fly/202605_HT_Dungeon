use std::sync::Arc;
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};
use egui_wgpu::ScreenDescriptor;

use ht_dungeon_core::entity::EntityKind;
use ht_dungeon_core::snapshot::RenderSnapshot;
use ht_dungeon_core::dungeon::TileKind;

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

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct QuadInstance {
    pub world_pos: [f32; 2],
    pub size:      f32,
    pub _pad:      f32,
    pub color:     [f32; 4],
}

impl QuadInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0,  shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 8,  shader_location: 2, format: wgpu::VertexFormat::Float32   },
                wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32   },
                wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
            ],
        }
    }
}

const QUAD_VERTS: &[[f32; 2]] = &[
    [-0.5, -0.5], [0.5, -0.5], [0.5, 0.5],
    [-0.5, -0.5], [0.5, 0.5], [-0.5, 0.5],
];

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device:  wgpu::Device,
    pub queue:   wgpu::Queue,
    pub config:  wgpu::SurfaceConfiguration,
    pub size:    winit::dpi::PhysicalSize<u32>,

    pipeline:     wgpu::RenderPipeline,
    vertex_buf:   wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    camera_buf:   wgpu::Buffer,
    camera_bg:    wgpu::BindGroup,

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
        let surface_format = caps.formats.iter().copied()
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&camera_bgl],
            push_constant_ranges: &[],
        });

        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad_verts"),
            contents: bytemuck::cast_slice(QUAD_VERTS),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_buf"),
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

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            vertex_buf,
            instance_buf,
            instance_cap: 0,
            camera_buf,
            camera_bg,
            egui_renderer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 { return; }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(
        &mut self,
        snapshot: &RenderSnapshot,
        camera: &Camera,
        egui_ctx: &egui::Context,
        egui_output: egui::FullOutput,
        pixels_per_point: f32,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = self.size.width as f32 / self.size.height as f32;
        let vp: [[f32; 4]; 4] = camera.view_proj(aspect);
        let vp_flat: [f32; 16] = unsafe { std::mem::transmute(vp) };
        self.queue.write_buffer(&self.camera_buf, 0, bytemuck::cast_slice(&vp_flat));

        let instances = build_instances(snapshot);
        self.upload_instances(&instances);

        // Egui texture updates and tessellation
        for (id, delta) in &egui_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, delta);
        }
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point,
        };
        let paint_jobs = egui_ctx.tessellate(egui_output.shapes, pixels_per_point);

        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("frame_encoder") }
        );

        self.egui_renderer.update_buffers(
            &self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor,
        );

        // Single pass: clear → world quads → egui overlay.
        // Using two separate passes on the same surface texture confuses the D3D12
        // backend's RENDER_TARGET→PRESENT barrier insertion, so we do everything here.
        {
            let color_attach = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.04, b: 0.04, a: 1.0 }),
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

            if !instances.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                pass.draw(0..6, 0..instances.len() as u32);
            }

            // egui_wgpu 0.29 requires RenderPass<'static>; raw-pointer cast is sound
            // because the pass is never stored and outlives this block.
            unsafe {
                let p: *mut wgpu::RenderPass<'static> =
                    &mut pass as *mut wgpu::RenderPass<'_> as *mut wgpu::RenderPass<'static>;
                self.egui_renderer.render(&mut *p, &paint_jobs, &screen_descriptor);
            }
        }

        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    fn upload_instances(&mut self, instances: &[QuadInstance]) {
        if instances.is_empty() { return; }
        let needed = instances.len();
        if needed > self.instance_cap {
            self.instance_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("instance_buf"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            self.instance_cap = needed;
        } else {
            self.queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(instances));
        }
    }
}

fn build_instances(snapshot: &RenderSnapshot) -> Vec<QuadInstance> {
    let mut out = Vec::with_capacity(
        snapshot.tiles.len() + snapshot.entities.len() + snapshot.items.len()
    );

    for tile in &snapshot.tiles {
        let color = match tile.kind {
            TileKind::Floor => [0.22f32, 0.18, 0.15, 1.0],
            TileKind::Wall  => [0.48f32, 0.46, 0.42, 1.0],
        };
        out.push(QuadInstance {
            world_pos: [tile.x as f32 + 0.5, tile.y as f32 + 0.5],
            size: 1.0, _pad: 0.0, color,
        });
    }

    for item in &snapshot.items {
        out.push(QuadInstance {
            world_pos: [item.position.x, item.position.y],
            size: 0.30, _pad: 0.0,
            color: [0.85, 0.75, 0.20, 1.0],
        });
    }

    for e in &snapshot.entities {
        if !e.alive { continue; }
        let (color, size) = match e.kind {
            EntityKind::Hero => ([0.20f32, 0.55, 0.90, 1.0], 0.55f32),
            EntityKind::Rat  => ([0.75f32, 0.35, 0.20, 1.0], 0.45f32),
        };
        out.push(QuadInstance {
            world_pos: [e.position.x, e.position.y],
            size, _pad: 0.0, color,
        });
    }

    out
}
