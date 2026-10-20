mod app;
mod filter;
mod keymaps;
mod reader;
mod sorter;
mod ui;

use crate::ui::render;
use reader::ParquetPreview;

use crossterm::event;
use ratatui::widgets::TableState;

struct App {
    preview: ParquetPreview,
    table_state: TableState,
    show_keymaps: bool,
    show_user_input: bool,
    user_input: String,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let parquet_file_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./data/sample-users.parquet".to_string());

    let mut app = App::new(&parquet_file_path)?;

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| {
                render(
                    frame,
                    &mut app.table_state,
                    &app.preview,
                    app.show_keymaps,
                    app.show_user_input,
                    &app.user_input,
                );
            })?;

            if let Some(key) = event::read()?.as_key_press_event()
                && app.handle_key(key)?
            {
                return Ok(());
            }
        }
    })
}
