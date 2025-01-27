use pollster::block_on;
use std::{borrow::Cow, io::Write as _, sync::Arc, time::SystemTime};
use util::{BufferInitDescriptor, DeviceExt as _};
use wgpu::*;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

#[repr(C)]
struct Vertex([f32; 2]);

impl Vertex {
    fn desc() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x2,
            }],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex([-1.0, -1.0]), // Top left
    Vertex([1.0, -1.0]),  // Top right
    Vertex([1.0, 1.0]),   // Bottom right
    Vertex([-1.0, 1.0]),  // Bottom left
];

const INDICES: &[[u16; 3]; 2] = &[
    [0, 1, 2], // Top right face
    [2, 3, 0], // Bottom left face
];

#[repr(C, align(16))] // The internet says 8, but the compiler says 16.
#[derive(Clone, Copy)]
struct PointPosition([f32; 2]);

const STARTING_POSITION: &[PointPosition; 4] = &[
    PointPosition([-0.5, -0.5]), // White
    PointPosition([0.5, -0.5]),  // Red
    PointPosition([0.5, 0.5]),   // Green
    PointPosition([-0.5, 0.5]),  // Blue
];

#[repr(C, align(16))]
struct PointColor([f32; 3]);

const STARTING_COLOR: &[PointColor; 4] = &[
    PointColor([1.0, 1.0, 1.0]), // White
    PointColor([1.0, 0.0, 0.0]), // Red
    PointColor([0.0, 1.0, 0.0]), // Green
    PointColor([0.0, 0.0, 1.0]), // Blue
];

// The ordering of this struct is important to the program's shutdown process.
pub(crate) struct Context {
    time_last_print: SystemTime,
    time_last_draw: SystemTime,
    time_start: SystemTime,
    points_position: [PointPosition; 4],
    points_position_buffer: Buffer,
    points_bind_group: BindGroup,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    queue: Queue,
    device: Device,
    render_pipeline: RenderPipeline,
    
    // SAFETY:
    // This MUST be dropped BEFORE window.
    // Wayland will segfault otherwise.
    // `surface` drops its strong reference
    // before it *fully* drops itself.
    // See [here](https://github.com/gfx-rs/wgpu/pull/6997).
    surface: Surface<'static>,
    window: Arc<Window>,
    config: SurfaceConfiguration,
}

impl Context {
    pub(crate) fn new(event_loop: &ActiveEventLoop) -> Self {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_inner_size(LogicalSize {
                            width: 500,
                            height: 500,
                        })
                        .with_resizable(false),
                )
                .unwrap(),
        );

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::from_env_or_default(),
            backend_options: BackendOptions::from_env_or_default(),
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap();

        let (device, queue) = block_on(adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::default().using_resolution(adapter.limits()),
                memory_hints: MemoryHints::MemoryUsage,
            },
            None,
        ))
        .unwrap();

        let vertex_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Glsl {
                shader: Cow::Borrowed(include_str!("shader.vert")),
                stage: naga::ShaderStage::Vertex,
                defines: Default::default(),
            },
        });

        let fragment_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Glsl {
                shader: Cow::Borrowed(include_str!("shader.frag")),
                stage: naga::ShaderStage::Fragment,
                defines: Default::default(),
            },
        });

        let points_position_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: std::mem::size_of_val(STARTING_POSITION) as u64,
            mapped_at_creation: false,
        });

        let points_color_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            contents: unsafe {
                std::slice::from_raw_parts(
                    STARTING_COLOR.as_ptr() as *const u8,
                    std::mem::size_of_val(STARTING_COLOR),
                )
            },
        });

        let points_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let points_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &points_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: points_position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: points_color_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&points_bind_group_layout],
            push_constant_ranges: &[],
        });

        let surface_capabilities = surface.get_capabilities(&adapter);
        let surface_format = surface_capabilities.formats[0];

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &vertex_shader,
                entry_point: None,
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &fragment_shader,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(surface_format.into())],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        config.present_mode = PresentMode::Mailbox;
        surface.configure(&device, &config);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: unsafe {
                std::slice::from_raw_parts(
                    VERTICES.as_ptr() as *const u8,
                    std::mem::size_of_val(VERTICES),
                )
            },
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: unsafe {
                std::slice::from_raw_parts(
                    INDICES.as_ptr() as *const u8,
                    std::mem::size_of_val(INDICES),
                )
            },
            usage: BufferUsages::INDEX,
        });

        let points_position = *STARTING_POSITION;

        let time_initial = SystemTime::now();

        Context {
            window,
            surface,
            config,
            device,
            queue,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            points_position,
            points_position_buffer,
            points_bind_group,
            time_start: time_initial,
            time_last_draw: time_initial,
            time_last_print: time_initial,
        }
    }

    pub(crate) fn redraw(&mut self) {
        let r = -SystemTime::now()
            .duration_since(self.time_start)
            .unwrap()
            .as_secs_f32()
            * std::f32::consts::TAU
            / 4.0;
        let mut sin_r = r.sin();
        let mut cos_r = r.cos();
        let scale_factor = (sin_r + 1.0) / 2.0;
        sin_r *= scale_factor;
        cos_r *= scale_factor;
        for i in 0..STARTING_POSITION.len() {
            self.points_position[i].0 = [
                (STARTING_POSITION[i].0[0] * cos_r - STARTING_POSITION[i].0[1] * sin_r),
                (STARTING_POSITION[i].0[0] * sin_r + STARTING_POSITION[i].0[1] * cos_r),
            ];
        }
        self.queue
            .write_buffer(&self.points_position_buffer, 0, unsafe {
                std::slice::from_raw_parts(
                    self.points_position.as_ptr() as *const u8,
                    std::mem::size_of_val(&self.points_position),
                )
            });

        let frame = self.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.points_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..(INDICES.len() * 3) as u32, 0, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        let time_current = SystemTime::now();
        if time_current.duration_since(self.time_last_print).unwrap()
            > std::time::Duration::from_millis(50)
        {
            print!(
                "\x1b[s{:7.1}\x1b[u",
                1.0 / time_current
                    .duration_since(self.time_last_draw)
                    .unwrap()
                    .as_secs_f32()
            );
            std::io::stdout().flush().unwrap();
            self.time_last_print = time_current;
        }
        self.time_last_draw = time_current;

        self.window.request_redraw();
    }

    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }
}
