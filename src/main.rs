mod reader;
mod sorter;
mod ui;

use crate::ui::render;
use reader::ParquetPreview;

use color_eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::widgets::TableState;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut preview = ParquetPreview::from_file("./data/sample-users.parquet", 15)?;

    color_eyre::install()?;

    let mut table_state = TableState::default();

    table_state.select_first();
    table_state.select_first_column();

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| render(frame, &mut table_state, &preview))?;
            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => table_state.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => table_state.select_previous(),
                    KeyCode::Char('l') | KeyCode::Right => table_state.select_next_column(),
                    KeyCode::Char('h') | KeyCode::Left => table_state.select_previous_column(),
                    KeyCode::Char('g') => table_state.select_first(),
                    KeyCode::Char('G') => table_state.select_last(),
                    KeyCode::PageDown => {
                        preview.next_batch()?;
                        table_state.select_first();
                    }
                    KeyCode::PageUp => {
                        preview.previous_batch()?;
                        table_state.select_first();
                    }
                    KeyCode::Char('s') => {
                        let col = table_state.selected_column().unwrap_or(0);
                        preview.cycle_sort(col)?;
                    }
                    _ => {}
                }
            }
        }
    })
}
