mod app;
mod filter;
mod keymaps;
mod reader;
mod sorter;
mod ui;

use crate::app::App;
use crate::ui::render;

use crossterm::event;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let parquet_file_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./data/sample-users.parquet".to_string());

    let mut app = App::new(&parquet_file_path)?;

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
