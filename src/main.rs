mod app;
mod filter;
mod keymaps;
mod reader;
mod sorter;
mod ui;

use std::path::PathBuf;

use crate::app::App;
use crate::ui::render;

use clap::Parser;
use crossterm::event;

#[derive(Parser)]
#[command( version, long_about = None)]
struct Cli {
    filename: PathBuf,

    /// Show schema only
    #[arg(short, long)]
    schema: bool,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let mut app = App::new(&cli.filename)?;

    if cli.schema {
        println!("{}", app.preview.schema());
        return Ok(());
    }

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| {
                render(frame, &mut app);
            })?;

            if let Some(key) = event::read()?.as_key_press_event()
                && app.handle_key(key)?
            {
                return Ok(());
            }
        }
    })
}
