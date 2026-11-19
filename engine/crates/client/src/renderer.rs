use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use revival_core::{Part, PartShape, SkyConfig};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PartInstance {
    pub pos: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 3],
    pub rot_x: [f32; 3],
    pub rot_y: [f32; 3],
    pub rot_z: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

#[derive(Copy, Clone, Default)]
pub struct HudState {
    pub show_touch_hud: bool,
    pub stick_center_uv: [f32; 2],
    pub stick_knob_norm: [f32; 2],
    pub stick_active: bool,
    pub jump_pressed: bool,
    pub screen_width: f32,
    pub screen_height: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct FrameUniform {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub light_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub sun_dir: [f32; 4],
    pub sky_top: [f32; 4],
    pub sky_bottom: [f32; 4],
    pub ambient: [f32; 4],
    pub hud_stick: [f32; 4],
    pub hud_screen: [f32; 4],
}

impl PartInstance {
    pub fn from_part(p: &Part) -> Self {
        Self {
            pos: p.position.to_array(),
            size: p.size.to_array(),
            color: p.color,
            rot_x: p.rotation[0],
            rot_y: p.rotation[1],
            rot_z: p.rotation[2],
        }
    }
}

pub fn build_camera_matrix(eye: Vec3, target: Vec3, aspect: f32) -> CameraUniform {
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let proj = Mat4::perspective_rh(65.0_f32.to_radians(), aspect.max(0.1), 0.2, 1200.0);
    CameraUniform {
        view_proj: (proj * view).to_cols_array_2d(),
    }
}

pub fn build_frame_uniform(
    eye: Vec3,
    target: Vec3,
    aspect: f32,
    sky: &SkyConfig,
    time_secs: f32,
    hud: HudState,
) -> FrameUniform {
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let proj = Mat4::perspective_rh(65.0_f32.to_radians(), aspect.max(0.1), 0.2, 1200.0);
    let vp = proj * view;
    let inv_vp = vp.inverse();

    let sun = sky.sun_direction.normalize_or_zero();
    let sun = if sun.length_squared() < 0.01 {
        Vec3::new(0.42, 0.85, 0.32).normalize()
    } else {
        sun
    };

    // Snap shadow frustum center to 1-stud grid around target to prevent shadow shimmer
    let focus = Vec3::new(target.x.round(), target.y.round(), target.z.round());
    let light_pos = focus + sun * 160.0;
    let light_up = if sun.y.abs() > 0.96 { Vec3::Z } else { Vec3::Y };
    let light_view = Mat4::look_at_rh(light_pos, focus, light_up);
    let light_proj = Mat4::orthographic_rh(-110.0, 110.0, -110.0, 110.0, 10.0, 380.0);
    let light_vp = light_proj * light_view;

    FrameUniform {
        view_proj: vp.to_cols_array_2d(),
        inv_view_proj: inv_vp.to_cols_array_2d(),
        light_view_proj: light_vp.to_cols_array_2d(),
        camera_pos: [eye.x, eye.y, eye.z, 1.0],
        sun_dir: [sun.x, sun.y, sun.z, sky.brightness],
        sky_top: [sky.sky_top[0], sky.sky_top[1], sky.sky_top[2], time_secs],
        sky_bottom: [sky.sky_bottom[0], sky.sky_bottom[1], sky.sky_bottom[2], 520.0],
        ambient: [sky.ambient[0], sky.ambient[1], sky.ambient[2], 1.0],
        hud_stick: [
            hud.stick_center_uv[0],
            hud.stick_center_uv[1],
            hud.stick_knob_norm[0],
            hud.stick_knob_norm[1],
        ],
        hud_screen: [
            hud.screen_width,
            hud.screen_height,
            if hud.stick_active { 1.0 } else { 0.0 },
            if hud.jump_pressed { 1.0 } else { 0.0 },
        ],
    }
}

pub fn unit_cube_vertices() -> ([Vertex; 24], [u16; 36]) {
    let p = 0.5_f32;
    let vertices = [
        // +Z
        Vertex { position: [-p, -p, p], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [p, -p, p], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [p, p, p], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [-p, p, p], normal: [0.0, 0.0, 1.0] },
        // -Z
        Vertex { position: [p, -p, -p], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-p, -p, -p], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-p, p, -p], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [p, p, -p], normal: [0.0, 0.0, -1.0] },
        // +Y
        Vertex { position: [-p, p, p], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [p, p, p], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [p, p, -p], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [-p, p, -p], normal: [0.0, 1.0, 0.0] },
        // -Y
        Vertex { position: [-p, -p, -p], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [p, -p, -p], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [p, -p, p], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [-p, -p, p], normal: [0.0, -1.0, 0.0] },
        // +X
        Vertex { position: [p, -p, p], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [p, -p, -p], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [p, p, -p], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [p, p, p], normal: [1.0, 0.0, 0.0] },
        // -X
        Vertex { position: [-p, -p, -p], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-p, -p, p], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-p, p, p], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-p, p, -p], normal: [-1.0, 0.0, 0.0] },
    ];
    let indices: [u16; 36] = [
        0, 1, 2, 2, 3, 0,
        4, 5, 6, 6, 7, 4,
        8, 9, 10, 10, 11, 8,
        12, 13, 14, 14, 15, 12,
        16, 17, 18, 18, 19, 16,
        20, 21, 22, 22, 23, 20,
    ];
    (vertices, indices)
}

fn unit_wedge_vertices() -> (Vec<Vertex>, Vec<u16>) {
    let p = 0.5_f32;
    let slope_n = Vec3::new(0.0, 1.0, 1.0).normalize().to_array();
    let vertices = vec![
        // Bottom (-Y)
        Vertex { position: [-p, -p, -p], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [p, -p, -p], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [p, -p, p], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [-p, -p, p], normal: [0.0, -1.0, 0.0] },
        // Back (-Z, full height)
        Vertex { position: [p, -p, -p], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-p, -p, -p], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-p, p, -p], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [p, p, -p], normal: [0.0, 0.0, -1.0] },
        // Slope (from -Z/+Y down to +Z/-Y)
        Vertex { position: [-p, -p, p], normal: slope_n },
        Vertex { position: [p, -p, p], normal: slope_n },
        Vertex { position: [p, p, -p], normal: slope_n },
        Vertex { position: [-p, p, -p], normal: slope_n },
        // Right (+X triangle)
        Vertex { position: [p, -p, p], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [p, -p, -p], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [p, p, -p], normal: [1.0, 0.0, 0.0] },
        // Left (-X triangle)
        Vertex { position: [-p, -p, -p], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-p, -p, p], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-p, p, -p], normal: [-1.0, 0.0, 0.0] },
    ];
    let indices = vec![
        0, 1, 2, 2, 3, 0,
        4, 5, 6, 6, 7, 4,
        8, 9, 10, 10, 11, 8,
        12, 13, 14,
        15, 16, 17,
    ];
    (vertices, indices)
}

fn unit_sphere_vertices() -> (Vec<Vertex>, Vec<u16>) {
    let stacks = 10_u16;
    let slices = 16_u16;
    let mut verts = Vec::new();
    let mut inds = Vec::new();
    for i in 0..=stacks {
        let phi = std::f32::consts::PI * (i as f32 / stacks as f32);
        let y = 0.5 * phi.cos();
        let r = 0.5 * phi.sin();
        for j in 0..=slices {
            let theta = std::f32::consts::TAU * (j as f32 / slices as f32);
            let x = r * theta.cos();
            let z = r * theta.sin();
            let n = Vec3::new(x, y, z).normalize_or_zero();
            verts.push(Vertex {
                position: [x, y, z],
                normal: n.to_array(),
            });
        }
    }
    let stride = slices + 1;
    for i in 0..stacks {
        for j in 0..slices {
            let a = i * stride + j;
            let b = a + stride;
            let c = b + 1;
            let d = a + 1;
            inds.extend_from_slice(&[a, d, b, d, c, b]);
        }
    }
    (verts, inds)
}

fn unit_cylinder_vertices() -> (Vec<Vertex>, Vec<u16>) {
    let segs = 16_u16;
    let mut verts = Vec::new();
    let mut inds = Vec::new();

    // Side ring
    for i in 0..=segs {
        let a = std::f32::consts::TAU * (i as f32 / segs as f32);
        let (s, c) = a.sin_cos();
        let n = [c, 0.0, s];
        verts.push(Vertex { position: [0.5 * c, -0.5, 0.5 * s], normal: n });
        verts.push(Vertex { position: [0.5 * c, 0.5, 0.5 * s], normal: n });
    }
    for i in 0..segs {
        let base = i * 2;
        inds.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
    }

    // Top cap (+Y)
    let top_center = verts.len() as u16;
    verts.push(Vertex { position: [0.0, 0.5, 0.0], normal: [0.0, 1.0, 0.0] });
    for i in 0..=segs {
        let a = std::f32::consts::TAU * (i as f32 / segs as f32);
        let (s, c) = a.sin_cos();
        verts.push(Vertex { position: [0.5 * c, 0.5, 0.5 * s], normal: [0.0, 1.0, 0.0] });
    }
    for i in 0..segs {
        inds.extend_from_slice(&[top_center, top_center + 2 + i, top_center + 1 + i]);
    }

    // Bottom cap (-Y)
    let bot_center = verts.len() as u16;
    verts.push(Vertex { position: [0.0, -0.5, 0.0], normal: [0.0, -1.0, 0.0] });
    for i in 0..=segs {
        let a = std::f32::consts::TAU * (i as f32 / segs as f32);
        let (s, c) = a.sin_cos();
        verts.push(Vertex { position: [0.5 * c, -0.5, 0.5 * s], normal: [0.0, -1.0, 0.0] });
    }
    for i in 0..segs {
        inds.extend_from_slice(&[bot_center, bot_center + 1 + i, bot_center + 2 + i]);
    }

    (verts, inds)
}

struct MeshGpu {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: u32,
}

impl MeshGpu {
    fn new(device: &wgpu::Device, label: &str, verts: &[Vertex], inds: &[u16]) -> Self {
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(inds),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buf,
            index_buf,
            index_count: inds.len() as u32,
        }
    }
}

pub struct PartPipeline {
    shadow_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    part_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    meshes: [MeshGpu; 4], // Block, Wedge, Ball, Cylinder
    frame_buf: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    shadow_bind_group: wgpu::BindGroup,
    instance_buf: wgpu::Buffer,
    instance_capacity: usize,
    shape_buckets: [Vec<PartInstance>; 4],
    merged_instances: Vec<PartInstance>,
}

impl PartPipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let (c_v, c_i) = unit_cube_vertices();
        let (w_v, w_i) = unit_wedge_vertices();
        let (s_v, s_i) = unit_sphere_vertices();
        let (y_v, y_i) = unit_cylinder_vertices();
        let meshes = [
            MeshGpu::new(device, "mesh-block", &c_v, &c_i),
            MeshGpu::new(device, "mesh-wedge", &w_v, &w_i),
            MeshGpu::new(device, "mesh-ball", &s_v, &s_i),
            MeshGpu::new(device, "mesh-cylinder", &y_v, &y_i),
        ];

        let init_uniform = build_frame_uniform(
            Vec3::new(16.0, 12.0, 16.0),
            Vec3::ZERO,
            16.0 / 9.0,
            &SkyConfig::default(),
            0.0,
            HudState::default(),
        );
        let frame_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame-ub"),
            contents: bytemuck::bytes_of(&init_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            }],
        });

        // 2048x2048 directional shadow map
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow-map"),
            size: wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-bg"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        let vertex_buffers = [
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
            },
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<PartInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    2 => Float32x3,
                    3 => Float32x3,
                    4 => Float32x3,
                    5 => Float32x3,
                    6 => Float32x3,
                    7 => Float32x3
                ],
            },
        ];

        let frame_only_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame-only-layout"),
            bind_group_layouts: &[&frame_bgl],
            push_constant_ranges: &[],
        });

        let part_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("part-pipeline-layout"),
            bind_group_layouts: &[&frame_bgl, &shadow_bgl],
            push_constant_ranges: &[],
        });

        // 1. Shadow depth pipeline
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow-pipeline"),
            layout: Some(&frame_only_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 2. Skybox pipeline
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky-pipeline"),
            layout: Some(&frame_only_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 3. Main part pipeline
        let part_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("part-pipeline"),
            layout: Some(&part_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 4. Mobile HUD overlay pipeline (alpha blended, no depth)
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud-pipeline"),
            layout: Some(&frame_only_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_hud"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_hud"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let instance_capacity = 512;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("part-instances"),
            size: (instance_capacity * std::mem::size_of::<PartInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            shadow_pipeline,
            sky_pipeline,
            part_pipeline,
            hud_pipeline,
            meshes,
            frame_buf,
            frame_bind_group,
            shadow_view,
            shadow_bind_group,
            instance_buf,
            instance_capacity,
            shape_buckets: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            merged_instances: Vec::with_capacity(instance_capacity),
        }
    }

    pub fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth-tex"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn render_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        parts: &[Part],
        frame: FrameUniform,
        draw_hud: bool,
    ) {
        queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));

        for b in &mut self.shape_buckets {
            b.clear();
        }
        for p in parts {
            if p.transparency < 0.95 {
                let idx = match p.shape {
                    PartShape::Block => 0,
                    PartShape::Wedge => 1,
                    PartShape::Ball => 2,
                    PartShape::Cylinder => 3,
                };
                self.shape_buckets[idx].push(PartInstance::from_part(p));
            }
        }

        self.merged_instances.clear();
        let mut ranges = [(0u32, 0u32); 4];
        for (i, bucket) in self.shape_buckets.iter().enumerate() {
            let start = self.merged_instances.len() as u32;
            self.merged_instances.extend_from_slice(bucket);
            let end = self.merged_instances.len() as u32;
            ranges[i] = (start, end);
        }

        let total = self.merged_instances.len();
        if total > self.instance_capacity {
            self.instance_capacity = total.next_power_of_two();
            self.instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("part-instances"),
                size: (self.instance_capacity * std::mem::size_of::<PartInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if total > 0 {
            queue.write_buffer(
                &self.instance_buf,
                0,
                bytemuck::cast_slice(&self.merged_instances),
            );
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("classic-roblox-encoder"),
        });

        // Pass 1: Directional sun shadow map
        {
            let mut spass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if total > 0 {
                spass.set_pipeline(&self.shadow_pipeline);
                spass.set_bind_group(0, &self.frame_bind_group, &[]);
                spass.set_vertex_buffer(1, self.instance_buf.slice(..));
                for (i, &(start, end)) in ranges.iter().enumerate() {
                    if end > start {
                        let mesh = &self.meshes[i];
                        spass.set_vertex_buffer(0, mesh.vertex_buf.slice(..));
                        spass.set_index_buffer(mesh.index_buf.slice(..), wgpu::IndexFormat::Uint16);
                        spass.draw_indexed(0..mesh.index_count, 0, start..end);
                    }
                }
            }
        }

        // Pass 2: Skybox + Lit & Shadowed Parts + Optional Mobile HUD
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: frame.sky_top[0] as f64,
                            g: frame.sky_top[1] as f64,
                            b: frame.sky_top[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Procedural Classic Roblox sky dome + sun + clouds
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.draw(0..3, 0..1);

            // Instanced parts by shape
            if total > 0 {
                pass.set_pipeline(&self.part_pipeline);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.set_bind_group(1, &self.shadow_bind_group, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                for (i, &(start, end)) in ranges.iter().enumerate() {
                    if end > start {
                        let mesh = &self.meshes[i];
                        pass.set_vertex_buffer(0, mesh.vertex_buf.slice(..));
                        pass.set_index_buffer(mesh.index_buf.slice(..), wgpu::IndexFormat::Uint16);
                        pass.draw_indexed(0..mesh.index_count, 0, start..end);
                    }
                }
            }

            // Crisp mobile virtual thumbstick & jump button
            if draw_hud {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(0, &self.frame_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}

pub fn render_offscreen_png(
    parts: &[Part],
    sky: &SkyConfig,
    width: u32,
    height: u32,
    out_path: &std::path::Path,
) -> anyhow::Result<()> {
    let visible: Vec<&Part> = parts.iter().filter(|p| p.transparency < 0.95).collect();
    let (eye, target) = if visible.len() > 10 {
        let mut sum = Vec3::ZERO;
        let mut count = 0.0_f32;
        for p in &visible {
            if p.position.length() < 500.0 {
                sum += p.position;
                count += 1.0;
            }
        }
        let center = if count > 0.0 { sum / count } else { Vec3::ZERO };
        (center + Vec3::new(110.0, 75.0, 125.0), center + Vec3::new(0.0, 6.0, 0.0))
    } else {
        (Vec3::new(14.0, 10.0, 18.0), Vec3::new(0.0, 1.0, 0.0))
    };

    let aspect = width as f32 / height as f32;
    let cam = build_camera_matrix(eye, target, aspect);

    let frame = build_frame_uniform(eye, target, aspect, sky, 0.0, HudState::default());
    // Try real wgpu headless rendering first (for simple scenes)
    if visible.len() <= 10 {
        if let Ok(rgba) = try_wgpu_offscreen(parts, width, height, frame) {
            return write_png(out_path, width, height, &rgba);
        }
    }

    let cubemap = SkyboxCubemap::from_sky_config(sky);
    let rgba = software_rasterize(parts, sky, cubemap.as_ref(), width, height, cam);
    write_png(out_path, width, height, &rgba)
}

struct SkyFace {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl SkyFace {
    fn sample(&self, u: f32, v: f32) -> [u8; 3] {
        let x = ((u.clamp(0.0, 1.0)) * (self.width - 1) as f32) as u32;
        let y = ((v.clamp(0.0, 1.0)) * (self.height - 1) as f32) as u32;
        let idx = ((y * self.width + x) * 4) as usize;
        [self.rgba[idx], self.rgba[idx + 1], self.rgba[idx + 2]]
    }
}

struct SkyboxCubemap {
    bk: SkyFace,
    dn: SkyFace,
    ft: SkyFace,
    lf: SkyFace,
    rt: SkyFace,
    up: SkyFace,
}

impl SkyboxCubemap {
    fn parse_asset_id(url: &Option<String>) -> Option<String> {
        let s = url.as_deref()?;
        if let Some(pos) = s.find("id=") {
            let digits: String = s[pos + 3..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return Some(digits);
            }
        }
        if let Some(rest) = s.strip_prefix("rbxassetid://") {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return Some(digits);
            }
        }
        None
    }

    #[cfg(target_arch = "wasm32")]
    fn from_sky_config(_sky: &SkyConfig) -> Option<Self> {
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn from_sky_config(sky: &SkyConfig) -> Option<Self> {
        let ids = [
            Self::parse_asset_id(&sky.skybox_bk)?,
            Self::parse_asset_id(&sky.skybox_dn)?,
            Self::parse_asset_id(&sky.skybox_ft)?,
            Self::parse_asset_id(&sky.skybox_lf)?,
            Self::parse_asset_id(&sky.skybox_rt)?,
            Self::parse_asset_id(&sky.skybox_up)?,
        ];
        let joined = ids.join(",");
        let api_url = format!(
            "https://thumbnails.roblox.com/v1/assets?assetIds={joined}&size=512x512&format=Png"
        );
        let response = match ureq::get(&api_url)
            .timeout(std::time::Duration::from_secs(6))
            .call()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[skybox] thumbnail API request failed: {e}");
                return None;
            }
        };
        let resp: serde_json::Value = match serde_json::from_reader(response.into_reader()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[skybox] thumbnail API JSON parse failed: {e}");
                return None;
            }
        };

        let mut url_map = std::collections::HashMap::new();
        for item in resp.get("data")?.as_array()? {
            if let (Some(id), Some(img_url)) = (
                item.get("targetId").and_then(|v| v.as_u64()),
                item.get("imageUrl").and_then(|v| v.as_str()),
            ) {
                url_map.insert(id.to_string(), img_url.to_string());
            }
        }

        let fetch_face = |id: &str| -> Option<SkyFace> {
            let img_url = url_map.get(id)?;
            let mut bytes = Vec::new();
            ureq::get(img_url)
                .timeout(std::time::Duration::from_secs(6))
                .call()
                .ok()?
                .into_reader()
                .read_to_end(&mut bytes)
                .ok()?;
            let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
            let mut reader = decoder.read_info().ok()?;
            let mut buf = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut buf).ok()?;
            let rgba = match info.color_type {
                png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
                png::ColorType::Rgb => buf[..info.buffer_size()]
                    .chunks_exact(3)
                    .flat_map(|c| [c[0], c[1], c[2], 255])
                    .collect(),
                _ => return None,
            };
            Some(SkyFace {
                width: info.width,
                height: info.height,
                rgba,
            })
        };

        let cubemap = Self {
            bk: fetch_face(&ids[0])?,
            dn: fetch_face(&ids[1])?,
            ft: fetch_face(&ids[2])?,
            lf: fetch_face(&ids[3])?,
            rt: fetch_face(&ids[4])?,
            up: fetch_face(&ids[5])?,
        };
        eprintln!("[skybox] loaded 6 cubemap faces from .rbxl ({})", ids.join(","));
        Some(cubemap)
    }

    fn sample(&self, dir: Vec3) -> [u8; 3] {
        let ax = dir.x.abs();
        let ay = dir.y.abs();
        let az = dir.z.abs();
        if ax >= ay && ax >= az {
            if dir.x > 0.0 {
                self.rt.sample(0.5 - 0.5 * (dir.z / ax), 0.5 - 0.5 * (dir.y / ax))
            } else {
                self.lf.sample(0.5 + 0.5 * (dir.z / ax), 0.5 - 0.5 * (dir.y / ax))
            }
        } else if ay >= ax && ay >= az {
            if dir.y > 0.0 {
                self.up.sample(0.5 + 0.5 * (dir.x / ay), 0.5 + 0.5 * (dir.z / ay))
            } else {
                self.dn.sample(0.5 + 0.5 * (dir.x / ay), 0.5 - 0.5 * (dir.z / ay))
            }
        } else if dir.z > 0.0 {
            self.bk.sample(0.5 + 0.5 * (dir.x / az), 0.5 - 0.5 * (dir.y / az))
        } else {
            self.ft.sample(0.5 - 0.5 * (dir.x / az), 0.5 - 0.5 * (dir.y / az))
        }
    }
}

fn try_wgpu_offscreen(
    parts: &[Part],
    width: u32,
    height: u32,
    frame: FrameUniform,
) -> anyhow::Result<Vec<u8>> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: true,
        compatible_surface: None,
    }))
    .ok_or_else(|| anyhow::anyhow!("no wgpu adapter"))?;

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let color_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen-color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = PartPipeline::create_depth_view(&device, width, height);
    let mut pipeline = PartPipeline::new(&device, format);
    pipeline.render_frame(&device, &queue, &color_view, &depth_view, parts, frame, false);

    let padded_row = ((width * 4 + 255) / 256) * 256;
    let read_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color_tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &read_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(enc.finish()));

    let slice = read_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let start = (y * padded_row) as usize;
        out.extend_from_slice(&data[start..start + (width * 4) as usize]);
    }
    Ok(out)
}

fn software_rasterize(
    parts: &[Part],
    sky: &SkyConfig,
    cubemap: Option<&SkyboxCubemap>,
    width: u32,
    height: u32,
    cam: CameraUniform,
) -> Vec<u8> {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let vp = Mat4::from_cols_array_2d(&cam.view_proj);
    let inv_vp = vp.inverse();

    for y in 0..height {
        let t = y as f32 / height as f32;
        let ndc_y = 1.0 - 2.0 * ((y as f32 + 0.5) / height as f32);
        for x in 0..width {
            let i = (y * width + x) as usize;
            let [r, g, b] = if let Some(cm) = cubemap {
                let ndc_x = 2.0 * ((x as f32 + 0.5) / width as f32) - 1.0;
                let near = inv_vp.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
                let far = inv_vp.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
                cm.sample((far - near).normalize())
            } else {
                [
                    ((sky.sky_top[0] * (1.0 - t) + sky.sky_bottom[0] * t) * 255.0).clamp(0.0, 255.0) as u8,
                    ((sky.sky_top[1] * (1.0 - t) + sky.sky_bottom[1] * t) * 255.0).clamp(0.0, 255.0) as u8,
                    ((sky.sky_top[2] * (1.0 - t) + sky.sky_bottom[2] * t) * 255.0).clamp(0.0, 255.0) as u8,
                ]
            };
            rgba[i * 4] = r;
            rgba[i * 4 + 1] = g;
            rgba[i * 4 + 2] = b;
            rgba[i * 4 + 3] = 255;
        }
    }
    let mut zbuf = vec![f32::INFINITY; (width * height) as usize];
    let (verts, inds) = unit_cube_vertices();
    let light_dir = sky.sun_direction.normalize();

    for part in parts {
        if part.transparency >= 0.95 {
            continue;
        }
        let rot = glam::Mat3::from_cols_array_2d(&part.rotation);
        for tri in inds.chunks_exact(3) {
            let mut normals = [Vec3::ZERO; 3];
            let mut locals = [Vec3::ZERO; 3];
            let mut ndcs = [glam::Vec4::ZERO; 3];
            for k in 0..3 {
                let v = verts[tri[k] as usize];
                let local = Vec3::from(v.position) * part.size;
                let world = rot * local + part.position;
                let clip = vp * world.extend(1.0);
                if clip.w <= 0.1 {
                    ndcs[k].w = -1.0;
                } else {
                    ndcs[k] = glam::Vec4::new(
                        (clip.x / clip.w * 0.5 + 0.5) * width as f32,
                        (-clip.y / clip.w * 0.5 + 0.5) * height as f32,
                        clip.z / clip.w,
                        clip.w,
                    );
                }
                normals[k] = (rot * Vec3::from(v.normal)).normalize();
                locals[k] = local;
            }
            if ndcs.iter().any(|p| p.w < 0.0) {
                continue;
            }
            let n = normals[0];
            let is_top = Vec3::from(verts[tri[0] as usize].normal).y > 0.5;
            let diffuse = n.dot(light_dir).max(0.32);

            let min_x = ndcs.iter().map(|p| p.x).fold(f32::INFINITY, f32::min).max(0.0) as i32;
            let max_x = ndcs.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max).min((width - 1) as f32) as i32;
            let min_y = ndcs.iter().map(|p| p.y).fold(f32::INFINITY, f32::min).max(0.0) as i32;
            let max_y = ndcs.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max).min((height - 1) as f32) as i32;

            let area = (ndcs[1].x - ndcs[0].x) * (ndcs[2].y - ndcs[0].y)
                - (ndcs[1].y - ndcs[0].y) * (ndcs[2].x - ndcs[0].x);
            if area.abs() < 1e-4 {
                continue;
            }

            let inv_w = [1.0 / ndcs[0].w, 1.0 / ndcs[1].w, 1.0 / ndcs[2].w];

            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let fx = px as f32 + 0.5;
                    let fy = py as f32 + 0.5;
                    let w0 = ((ndcs[2].x - ndcs[1].x) * (fy - ndcs[1].y)
                        - (ndcs[2].y - ndcs[1].y) * (fx - ndcs[1].x))
                        / area;
                    let w1 = ((ndcs[0].x - ndcs[2].x) * (fy - ndcs[2].y)
                        - (ndcs[0].y - ndcs[2].y) * (fx - ndcs[2].x))
                        / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                        let z = w0 * ndcs[0].z + w1 * ndcs[1].z + w2 * ndcs[2].z;
                        if !(0.0..=1.0).contains(&z) {
                            continue;
                        }
                        let idx = (py as u32 * width + px as u32) as usize;
                        if z < zbuf[idx] {
                            zbuf[idx] = z;

                            // Perspective-correct local stud coordinates
                            let w_interp = 1.0 / (w0 * inv_w[0] + w1 * inv_w[1] + w2 * inv_w[2]);
                            let local = (locals[0] * (w0 * inv_w[0])
                                + locals[1] * (w1 * inv_w[1])
                                + locals[2] * (w2 * inv_w[2]))
                                * w_interp;

                            // Classic Roblox Stud texture on top surfaces (1 stud = 1.0 unit)
                            let mut shade = diffuse;
                            if is_top {
                                let sx = (local.x + part.size.x * 0.5).rem_euclid(1.0) - 0.5;
                                let sz = (local.z + part.size.z * 0.5).rem_euclid(1.0) - 0.5;
                                let r2 = sx * sx + sz * sz;
                                if r2 < 0.075 {
                                    shade *= 1.12;
                                } else if r2 < 0.115 {
                                    shade *= 0.86;
                                }
                            }

                            rgba[idx * 4] = (part.color[0] * shade * 255.0).clamp(0.0, 255.0) as u8;
                            rgba[idx * 4 + 1] = (part.color[1] * shade * 255.0).clamp(0.0, 255.0) as u8;
                            rgba[idx * 4 + 2] = (part.color[2] * shade * 255.0).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }
    rgba
}

fn write_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}
