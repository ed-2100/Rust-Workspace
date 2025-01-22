use std::{borrow::Cow, sync::Arc};

use wgpu::*;
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

#[allow(dead_code)]
pub(crate) struct Context {
    window: Arc<Window>,
    instance: Instance,
    surface: Surface<'static>,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    shader: ShaderModule,
    pipeline_layout: PipelineLayout,
    render_pipeline: RenderPipeline,
    config: SurfaceConfiguration,
}

impl Context {
    pub(crate) fn new(event_loop: &ActiveEventLoop) -> Self {
        let window = Arc::new(event_loop.create_window(Default::default()).unwrap());

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::VULKAN,
            flags: InstanceFlags::from_env_or_default(),
            backend_options: BackendOptions::from_env_or_default(),
        });

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits:
                    Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                memory_hints: MemoryHints::MemoryUsage,
            },
            None,
        ))
        .unwrap();

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        Context {
            window,
            instance,
            surface,
            adapter,
            device,
            queue,
            shader,
            pipeline_layout,
            render_pipeline,
            config,
        }
    }

    pub(crate) fn redraw(&mut self) {
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLUE),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }
}
