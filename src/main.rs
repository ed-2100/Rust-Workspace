use log::info;
use winit::event_loop::EventLoop;

mod app;
use app::Application;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    // Drop early for debugging purposes.
    {
        let mut app = Application::default();

        info!("Running...");
        event_loop.run_app(&mut app).unwrap();

        info!("Exiting...");
    }

    info!("Done.");
}
