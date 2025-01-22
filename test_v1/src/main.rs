use std::{error::Error, io::Write as _};

use winit::event_loop::EventLoop;

mod app;
use app::Application;

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
