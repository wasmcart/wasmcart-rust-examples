//! The renderer: pipelines, buffers and the per-frame passes (particle
//! compute, sky, lit instanced meshes, additive particle billboards, HUD).

use wasmcart_wgpu::{wgpu, Gpu};

use crate::font::{self, Quad};
use crate::meshes::{self, Mesh};
use crate::shaders;
use crate::{H, W};

pub const PARTICLES: u32 = 6144;
const PARTICLE_BYTES: u64 = 48;
const MAX_INST: usize = 4096;
const MAX_QUADS: usize = 4096;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

/// Mesh slots, in draw order.
pub const CANYON: usize = 0;
pub const CRYSTAL: usize = 1;
pub const RING: usize = 2;
pub const GLIDER: usize = 3;
pub const MESHES: usize = 4;

/// One lit mesh instance: model matrix, colour, params
/// (x emissive, y shading kind, z pulse phase, w extra glow).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Inst {
    pub m: [f32; 16],
    pub color: [f32; 4],
    pub params: [f32; 4],
}

/// The scene uniform, laid out as the WGSL `Scene` struct.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SceneU {
    pub view_proj: [f32; 16],
    pub cam: [f32; 4],
    pub sun_dir: [f32; 4],
    pub sun_col: [f32; 4],
    pub fog: [f32; 4],
    pub sky_top: [f32; 4],
    pub accent: [f32; 4],
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub lights: [[f32; 4]; 16],
}

/// The particle simulation uniform (WGSL `Sim`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SimU {
    pub a: [f32; 4],
    pub glider: [f32; 4],
    pub glider_vel: [f32; 4],
    pub mote_col: [f32; 4],
    pub trail_col: [f32; 4],
    pub bursts: [[f32; 4]; 16],
}

/// Everything the game hands the renderer for one frame.
pub struct FrameData {
    pub scene: SceneU,
    pub sim: SimU,
    pub inst: [Vec<Inst>; MESHES],
    pub clear: [f32; 3],
}

struct GpuMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    count: u32,
}

pub struct Renderer {
    pub gpu: Gpu,
    depth: wgpu::TextureView,
    scene_buf: wgpu::Buffer,
    sim_buf: wgpu::Buffer,
    particles: wgpu::Buffer,
    inst_buf: wgpu::Buffer,
    quad_buf: wgpu::Buffer,
    scene_group: wgpu::BindGroup,
    hud_group: wgpu::BindGroup,
    sim_group: wgpu::BindGroup,
    sim: wgpu::ComputePipeline,
    sky: wgpu::RenderPipeline,
    lit: wgpu::RenderPipeline,
    spark: wgpu::RenderPipeline,
    hud: wgpu::RenderPipeline,
    meshes: Vec<GpuMesh>,
}

pub fn bytes<T>(s: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

fn buffer_init(device: &wgpu::Device, label: &str, data: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    let size = (data.len() as u64 + 3) & !3;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: true });
    buf.slice(..).get_mapped_range_mut().expect("mapped at creation").slice(..data.len()).copy_from_slice(data);
    buf.unmap();
    buf
}

fn upload(device: &wgpu::Device, m: &Mesh, label: &str) -> GpuMesh {
    GpuMesh {
        vbuf: buffer_init(device, label, bytes(&m.verts), wgpu::BufferUsages::VERTEX),
        ibuf: buffer_init(device, label, bytes(&m.idx), wgpu::BufferUsages::INDEX),
        count: m.idx.len() as u32,
    }
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

impl Renderer {
    pub fn new(gpu: Gpu) -> Renderer {
        let device = &gpu.device;
        let shader = |label: &str, src: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(format!("{}{}", shaders::COMMON, src).into()),
            })
        };
        let lit_mod = shader("lit", shaders::LIT);
        let sky_mod = shader("sky", shaders::SKY);
        let spark_mod = shader("particles", shaders::PARTICLE);
        let hud_mod = shader("hud", shaders::HUD);
        let sim_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sim"),
            source: wgpu::ShaderSource::Wgsl(shaders::SIM.into()),
        });

        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());

        let ubuf = |label: &str, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let scene_buf = ubuf("scene", std::mem::size_of::<SceneU>());
        let sim_buf = ubuf("sim", std::mem::size_of::<SimU>());
        // Zeroed at creation: the simulation seeds every particle whose seq is 0.
        let particles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles"),
            size: PARTICLES as u64 * PARTICLE_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let inst_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (MAX_INST * std::mem::size_of::<Inst>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud quads"),
            size: (MAX_QUADS * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Font atlas.
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("font"),
            size: wgpu::Extent3d { width: font::ATLAS_W, height: font::ATLAS_H, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            atlas.as_image_copy(),
            &font::atlas(),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(font::ATLAS_W * 4), rows_per_image: Some(font::ATLAS_H) },
            wgpu::Extent3d { width: font::ATLAS_W, height: font::ATLAS_H, depth_or_array_layers: 1 },
        );
        let atlas_view = atlas.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let all = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &[uniform_entry(0, all)],
        });
        let hud_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud"),
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
        let sim_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sim"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene"),
            layout: &scene_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: scene_buf.as_entire_binding() }],
        });
        let hud_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud"),
            layout: &hud_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let sim_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sim"),
            layout: &sim_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: particles.as_entire_binding() },
            ],
        });

        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&scene_bgl)],
            immediate_size: 0,
        });
        let hud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud"),
            bind_group_layouts: &[Some(&scene_bgl), Some(&hud_bgl)],
            immediate_size: 0,
        });
        let sim_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sim"),
            bind_group_layouts: &[Some(&sim_bgl)],
            immediate_size: 0,
        });

        let sim = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sim"),
            layout: Some(&sim_layout),
            module: &sim_mod,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let fmt = gpu.view_format();
        let consts = gpu.srgb_constants();
        let depth_state = |write: bool, cmp: wgpu::CompareFunction| {
            Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(write),
                depth_compare: Some(cmp),
                stencil: Default::default(),
                bias: Default::default(),
            })
        };
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        module: &wgpu::ShaderModule,
                        buffers: &[Option<wgpu::VertexBufferLayout>],
                        blend: Option<wgpu::BlendState>,
                        depth: Option<wgpu::DepthStencilState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs"),
                    buffers,
                    compilation_options: wgpu::PipelineCompilationOptions { constants: consts, ..Default::default() },
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState { format: fmt, blend, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: wgpu::PipelineCompilationOptions { constants: consts, ..Default::default() },
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: depth,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let sky = pipeline("sky", &scene_layout, &sky_mod, &[], None, depth_state(false, wgpu::CompareFunction::Always));
        let lit = pipeline(
            "lit",
            &scene_layout,
            &lit_mod,
            &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Inst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
                    ],
                }),
            ],
            None,
            depth_state(true, wgpu::CompareFunction::Less),
        );
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        let spark = pipeline(
            "particles",
            &scene_layout,
            &spark_mod,
            &[Some(wgpu::VertexBufferLayout {
                array_stride: PARTICLE_BYTES,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
            })],
            Some(additive),
            depth_state(false, wgpu::CompareFunction::Less),
        );
        let hud = pipeline(
            "hud",
            &hud_layout,
            &hud_mod,
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Quad>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
            })],
            Some(wgpu::BlendState::ALPHA_BLENDING),
            depth_state(false, wgpu::CompareFunction::Always),
        );

        let meshes = vec![
            upload(device, &meshes::canyon(crate::world::SEG_LEN, crate::world::WALL_X, crate::world::FLOOR_Y, 9.0), "canyon"),
            upload(device, &meshes::crystal(), "crystal"),
            upload(device, &meshes::torus(0.14), "ring"),
            upload(device, &meshes::glider(), "glider"),
        ];

        Renderer {
            gpu,
            depth,
            scene_buf,
            sim_buf,
            particles,
            inst_buf,
            quad_buf,
            scene_group,
            hud_group,
            sim_group,
            sim,
            sky,
            lit,
            spark,
            hud,
            meshes,
        }
    }

    pub fn draw(&mut self, f: &FrameData, quads: &[Quad]) {
        let gpu = &self.gpu;
        gpu.queue.write_buffer(&self.scene_buf, 0, bytes(std::slice::from_ref(&f.scene)));
        gpu.queue.write_buffer(&self.sim_buf, 0, bytes(std::slice::from_ref(&f.sim)));

        // All instances in one buffer, one range per mesh.
        let mut all: Vec<Inst> = Vec::with_capacity(1024);
        let mut ranges = [(0u64, 0u32); MESHES];
        for (k, list) in f.inst.iter().enumerate() {
            let take = list.len().min(MAX_INST - all.len());
            ranges[k] = ((all.len() * std::mem::size_of::<Inst>()) as u64, take as u32);
            all.extend_from_slice(&list[..take]);
        }
        if !all.is_empty() {
            gpu.queue.write_buffer(&self.inst_buf, 0, bytes(&all));
        }
        let nq = quads.len().min(MAX_QUADS);
        if nq > 0 {
            gpu.queue.write_buffer(&self.quad_buf, 0, bytes(&quads[..nq]));
        }

        let Some(frame) = gpu.frame() else { return };
        let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sim"), timestamp_writes: None });
            cp.set_pipeline(&self.sim);
            cp.set_bind_group(0, &self.sim_group, &[]);
            cp.dispatch_workgroups(PARTICLES.div_ceil(64), 1, 1);
        }
        {
            let c = f.clear;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(gpu.clear_color(wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 })),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.scene_group, &[]);
            pass.set_pipeline(&self.sky);
            pass.draw(0..3, 0..1);

            pass.set_pipeline(&self.lit);
            for (k, m) in self.meshes.iter().enumerate() {
                let (off, n) = ranges[k];
                if n == 0 {
                    continue;
                }
                pass.set_vertex_buffer(0, m.vbuf.slice(..));
                pass.set_vertex_buffer(1, self.inst_buf.slice(off..));
                pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..m.count, 0, 0..n);
            }

            pass.set_pipeline(&self.spark);
            pass.set_vertex_buffer(0, self.particles.slice(..));
            pass.draw(0..6, 0..PARTICLES);

            if nq > 0 {
                pass.set_pipeline(&self.hud);
                pass.set_bind_group(1, &self.hud_group, &[]);
                pass.set_vertex_buffer(0, self.quad_buf.slice(..));
                pass.draw(0..6, 0..nq as u32);
            }
        }
        gpu.queue.submit([enc.finish()]);
        gpu.present(frame);
    }
}
