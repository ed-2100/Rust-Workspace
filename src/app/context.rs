use pollster::block_on;
use std::{
    borrow::Cow, collections::HashMap, io::Write as _, num::NonZero, sync::Arc, time::SystemTime,
};
use wgpu::*;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::ActiveEventLoop,
    window::{Fullscreen, Window, WindowAttributes},
};

const NUM_FRAMES: usize = 2;

#[repr(C, align(16))] // The internet says 8, but the compiler says 16.
#[derive(Clone, Copy)]
struct PointPosition([f32; 2]);

const STARTING_POSITION: &[PointPosition; 4] = &[
    PointPosition([-0.5, -0.5]), // White
    PointPosition([0.5, -0.5]),  // Red
    PointPosition([0.5, 0.5]),   // Green
    PointPosition([-0.5, 0.5]),  // Blue
];

struct FrameData {
    points_position_buffer: Buffer,
    texture: Texture,
    bind_group: BindGroup,
}

// The ordering of this struct is important to the program's shutdown process.
pub struct Context {
    time_last_print: SystemTime,
    time_last_draw: SystemTime,
    time_start: SystemTime,

    bind_group_layout: BindGroupLayout,
    frame_data: [FrameData; NUM_FRAMES],
    frame_data_index: usize,

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

// Public methods
impl Context {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let window = Self::create_window(event_loop);
        let size = Self::get_window_size(&window);

        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            flags: InstanceFlags::from_env_or_default(),
            backend_options: BackendOptions::from_env_or_default(),
        });

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = Self::request_adapter(&instance, &surface);
        let (device, queue) = Self::request_device(&adapter);

        let config = Self::configure_surface(&surface, &adapter, &device, size);
        let bind_group_layout = Self::create_bind_group_layout(&device);
        let frame_data = Self::create_frame_data(&device, &config, &bind_group_layout);

        let compute_pipeline = Self::create_compute_pipeline(&device, &bind_group_layout);

        let time_initial = SystemTime::now();

        Self {
            window,
            surface,
            config,
            device,
            queue,
            time_start: time_initial,
            time_last_draw: time_initial,
            time_last_print: time_initial,
            compute_pipeline,
            frame_data,
            frame_data_index: 0,
            bind_group_layout,
        }
    }

    pub fn redraw(&mut self) {
        let frame = self.surface.get_current_texture().unwrap();
        let frame_data = &self.frame_data[self.frame_data_index];

        self.update_points_position_buffer(frame_data);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        self.dispatch_compute_pass(&mut encoder, frame_data);

        encoder.copy_texture_to_texture(
            frame_data.texture.as_image_copy(),
            frame.texture.as_image_copy(),
            Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        self.update_timing();

        self.frame_data_index = (self.frame_data_index + 1) % self.frame_data.len();

        self.window.request_redraw();
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);

        for frame_data in self.frame_data.iter_mut() {
            frame_data.texture = self.device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
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

            let texture_view = frame_data
                .texture
                .create_view(&TextureViewDescriptor::default());

            frame_data.bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &self.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: frame_data.points_position_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&texture_view),
                    },
                ],
            });
        }

        self.window.request_redraw();
    }

    pub fn toggle_fullscreen(&self) {
        if self.window.fullscreen().is_some() {
            self.window.set_fullscreen(None);
        } else {
            self.window
                .set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
    }
}

// Private methods
impl Context {
    fn create_window(event_loop: &ActiveEventLoop) -> Arc<Window> {
        Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_inner_size(LogicalSize {
                            width: 500,
                            height: 500,
                        })
                        .with_resizable(true)
                        .with_title("glowing_dots"),
                )
                .unwrap(),
        )
    }

    fn get_window_size(window: &Window) -> PhysicalSize<u32> {
        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);
        size
    }

    fn request_adapter(instance: &Instance, surface: &Surface) -> Adapter {
        block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(surface),
        }))
        .unwrap()
    }

    fn request_device(adapter: &Adapter) -> (Device, Queue) {
        block_on(adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::default().using_resolution(adapter.limits()),
                memory_hints: MemoryHints::MemoryUsage,
            },
            None,
        ))
        .unwrap()
    }

    fn configure_surface(
        surface: &Surface,
        adapter: &Adapter,
        device: &Device,
        size: PhysicalSize<u32>,
    ) -> SurfaceConfiguration {
        let mut config = surface
            .get_default_config(adapter, size.width, size.height)
            .unwrap();
        config.format = TextureFormat::Rgba8Unorm;
        config.usage |= TextureUsages::COPY_DST;
        config.present_mode = if std::env::args().any(|arg| arg == "--turbo") {
            PresentMode::AutoNoVsync
        } else {
            PresentMode::AutoVsync
        };
        surface.configure(device, &config);
        config
    }

    fn create_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
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
        })
    }

    fn create_frame_data(
        device: &Device,
        config: &SurfaceConfiguration,
        bind_group_layout: &BindGroupLayout,
    ) -> [FrameData; NUM_FRAMES] {
        std::array::from_fn(|_| {
            let points_position_buffer = device.create_buffer(&BufferDescriptor {
                label: None,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                size: std::mem::size_of_val(STARTING_POSITION) as u64,
                mapped_at_creation: false,
            });

            let texture = device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
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

            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: points_position_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&texture_view),
                    },
                ],
            });

            FrameData {
                points_position_buffer,
                texture,
                bind_group,
            }
        })
    }

    fn create_compute_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
    ) -> ComputePipeline {
        let compute_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Glsl {
                shader: Cow::Borrowed(include_str!("shader.comp.glsl")),
                stage: naga::ShaderStage::Compute,
                defines: HashMap::default(),
            },
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &compute_shader,
            entry_point: None,
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        })
    }

    fn update_points_position_buffer(&self, frame_data: &FrameData) {
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

        let mut mapped = self
            .queue
            .write_buffer_with(
                &frame_data.points_position_buffer,
                0,
                NonZero::new(frame_data.points_position_buffer.size()).unwrap(),
            )
            .unwrap();

        let mapped_slice = unsafe {
            std::slice::from_raw_parts_mut(
                mapped.as_mut_ptr().cast::<PointPosition>(),
                (frame_data.points_position_buffer.size()
                    / std::mem::size_of::<PointPosition>() as u64) as usize,
            )
        };

        for (i, pos) in STARTING_POSITION.iter().enumerate() {
            mapped_slice[i].0 = [
                (pos.0[0] * cos_r - pos.0[1] * sin_r),
                (pos.0[0] * sin_r + pos.0[1] * cos_r),
            ];
        }
    }

    fn dispatch_compute_pass(&self, encoder: &mut CommandEncoder, frame_data: &FrameData) {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &frame_data.bind_group, &[]);

        let workgroup_size_x = 8;
        let workgroup_size_y = 8;

        let dispatch_x = self.config.width.div_ceil(workgroup_size_x);
        let dispatch_y = self.config.height.div_ceil(workgroup_size_y);

        compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }

    fn update_timing(&mut self) {
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
    }
}
