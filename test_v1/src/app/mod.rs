
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::ActiveEventLoop, keyboard::KeyCode, window::WindowId};

mod context;
use context::Context;

#[derive(Default)]
pub(crate) struct Application {
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
            WindowEvent::Resized(new_size) => context.resize(new_size),
            WindowEvent::RedrawRequested => context.redraw(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key == KeyCode::Escape && !event.repeat {
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        };
    }
}
