use std::{error::Error, io::Write as _};

use winit::event_loop::EventLoop;

mod app;
use app::Application;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    let mut app = Application::default();
    let mut stdout = std::io::stdout();

    print!("Running...");
    stdout.flush()?;

    event_loop.run_app(&mut app)?;

    println!(" Done.");
    stdout.flush()?;

    Ok(())
}
