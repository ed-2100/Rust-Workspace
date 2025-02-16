use pollster::block_on;
use std::{borrow::Cow, io::Write as _, sync::Arc, time::SystemTime};
use wgpu::*;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

#[repr(C, align(16))] // The internet says 8, but the compiler says 16.
#[derive(Clone, Copy)]
struct PointPosition([f32; 2]);

const STARTING_POSITION: &[PointPosition; 4] = &[
    PointPosition([-0.5, -0.5]), // White
    PointPosition([0.5, -0.5]),  // Red
    PointPosition([0.5, 0.5]),   // Green
    PointPosition([-0.5, 0.5]),  // Blue
];

// The ordering of this struct is important to the program's shutdown process.
pub(crate) struct Context {
    time_last_print: SystemTime,
    time_last_draw: SystemTime,
    time_start: SystemTime,
    points_position: [PointPosition; 4],
    points_position_buffer: Buffer,
    texture: Texture,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    queue: Queue,
    device: Device,
    compute_pipeline: ComputePipeline,

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
                        .with_resizable(true),
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

        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        config.format = TextureFormat::Rgba8Unorm;
        config.usage |= TextureUsages::COPY_DST;
        config.present_mode = PresentMode::AutoNoVsync;
        surface.configure(&device, &config);

        let compute_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Glsl {
                shader: Cow::Borrowed(include_str!("shader.comp")),
                stage: naga::ShaderStage::Compute,
                defines: Default::default(),
            },
        });

        let points_position_buffer = device.create_buffer(&BufferDescriptor {
            label: None,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: std::mem::size_of_val(STARTING_POSITION) as u64,
            mapped_at_creation: false,
        });

        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: points_position_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&texture_view),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &compute_shader,
            entry_point: None,
            compilation_options: Default::default(),
            cache: None,
        });

        let points_position = *STARTING_POSITION;

        let time_initial = SystemTime::now();

        Context {
            window,
            surface,
            config,
            device,
            queue,
            points_position,
            points_position_buffer,
            time_start: time_initial,
            time_last_draw: time_initial,
            time_last_print: time_initial,
            texture,
            compute_pipeline,
            bind_group,
            bind_group_layout,
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
        for (i, pos) in STARTING_POSITION.iter().enumerate() {
            self.points_position[i].0 = [
                (pos.0[0] * cos_r - pos.0[1] * sin_r),
                (pos.0[0] * sin_r + pos.0[1] * cos_r),
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
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.bind_group, &[]);

            let workgroup_size_x = 8;
            let workgroup_size_y = 8;

            let dispatch_x = self.config.width.div_ceil(workgroup_size_x);
            let dispatch_y = self.config.height.div_ceil(workgroup_size_y);

            compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        encoder.copy_texture_to_texture(
            self.texture.as_image_copy(),
            frame.texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

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

        self.texture = self.device.create_texture(&TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = self.texture.create_view(&TextureViewDescriptor::default());

        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.points_position_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&texture_view),
                },
            ],
        });
    }
}
