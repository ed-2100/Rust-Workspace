use std::{error::Error, io::Write as _};

use winit::event_loop::EventLoop;

mod app;
use app::Application;

fn main() -> Result<(), Box<dyn Error>> {
    // env_logger::init();
    let event_loop = EventLoop::new().unwrap();

    print!("Running...\n");
    std::io::stdout().flush().unwrap();

    // Drop early for debugging purposes.
    {
        let mut app = Application::default();
        event_loop.run_app(&mut app).unwrap();
    }

    print!("\nDone.\n");
    std::io::stdout().flush().unwrap();

    Ok(())
}
