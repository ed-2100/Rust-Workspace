use std::{borrow::Cow, error::Error, io::Write as _, sync::Arc};

use wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowId},
};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    let mut app = Application::default();

    print!("Running...");
    std::io::stdout().flush().unwrap();

    event_loop.run_app(&mut app)?;

    println!(" Done.");
    std::io::stdout().flush().unwrap();

    Ok(())
}

#[derive(Default)]
struct Application {
    context: Option<Context>,
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.context.is_none() {
            self.context = Some(Context::new(event_loop))
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let context = self.context.as_mut().unwrap();
        match event {
            WindowEvent::Resized(new_size) => {
                context.config.width = new_size.width.max(1);
                context.config.height = new_size.height.max(1);
                context.surface.configure(&context.device, &context.config);
                context.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = context.surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&TextureViewDescriptor::default());
                let mut encoder = context
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

                    rpass.set_pipeline(&context.render_pipeline);
                    rpass.draw(0..3, 0..1);
                }

                context.queue.submit(Some(encoder.finish()));
                frame.present();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key == KeyCode::Escape && !event.repeat {
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => {}
        };
    }
}

#[allow(dead_code)]
struct Context {
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
    fn new(event_loop: &ActiveEventLoop) -> Self {
        let window = Arc::new(event_loop.create_window(Default::default()).unwrap());

        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::VULKAN,
            flags: InstanceFlags::empty(),
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
}
