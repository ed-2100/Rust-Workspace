use log::info;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};

mod context;
use context::Context;

#[derive(Default)]
pub struct Application {
    context: Option<Context>,
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.context.is_none() {
            info!("Creating context...");
            self.context = Some(Context::new(event_loop));
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
            WindowEvent::Resized(new_size) => context.resize(new_size),
            WindowEvent::RedrawRequested => context.redraw(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let KeyEvent {
                    repeat: false,
                    state: ElementState::Pressed,
                    ..
                } = event
                {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            event_loop.exit();
                        }
                        PhysicalKey::Code(KeyCode::F11) => {
                            context.toggle_fullscreen();
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        };
    }
}
