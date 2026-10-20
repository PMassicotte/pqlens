mod filter;
mod keymaps;
mod reader;
mod sorter;
mod ui;

use crate::ui::render;
use color_eyre::eyre::Ok;
use reader::ParquetPreview;

use crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::widgets::TableState;

struct App {
    preview: ParquetPreview,
    table_state: TableState,
    show_keymaps: bool,
    show_user_input: bool,
    user_input: String,
}

impl App {
    fn new(path: &str) -> color_eyre::Result<App> {
        let mut app = App {
            preview: ParquetPreview::from_file(path, 45)?,
            table_state: TableState::default(),
            show_keymaps: false,
            show_user_input: false,
            user_input: String::new(),
        };

        app.table_state.select_first();
        app.table_state.select_first_column();

        Ok(app)
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> color_eyre::Result<bool> {
        if self.show_user_input {
            match key.code {
                KeyCode::Char(c) => self.user_input.push(c),
                KeyCode::Backspace => {
                    self.user_input.pop();
                }
                KeyCode::Esc => {
                    self.user_input = String::new();
                    self.show_user_input = false;
                }
                KeyCode::Enter => {
                    if self.user_input.is_empty() {
                        self.preview.clear_filter()?;
                    } else {
                        self.preview.set_filter(
                            self.table_state.selected_column().unwrap_or(0),
                            &self.user_input,
                        )?;
                    }

                    self.show_user_input = false;
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('c') => {
                    self.preview.clear_filter()?;
                    self.preview.clear_sort();
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    if self.show_keymaps {
                        self.show_keymaps = false;
                    } else {
                        return Ok(true);
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => self.table_state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.table_state.select_previous(),
                KeyCode::Char('l') | KeyCode::Right => self.table_state.select_next_column(),
                KeyCode::Char('h') | KeyCode::Left => self.table_state.select_previous_column(),
                KeyCode::Char('g') => self.table_state.select_first(),
                KeyCode::Char('G') => self.table_state.select_last(),
                KeyCode::Char('f') => {
                    if self.preview.is_filtered() {
                        self.preview.clear_filter()?;
                    } else {
                        let Some((row, col)) = self.table_state.selected_cell() else {
                            return Ok(false);
                        };

                        self.preview.filter_by_cell(row, col)?;
                    }
                }
                KeyCode::Char('?') => self.show_keymaps = !self.show_keymaps,
                KeyCode::PageDown => {
                    self.preview.next_view()?;
                    self.table_state.select_first();
                }
                KeyCode::PageUp => {
                    self.preview.previous_view()?;
                    self.table_state.select_first();
                }
                KeyCode::Home => {
                    self.preview.first_view()?;
                    self.table_state.select_first();
                }
                KeyCode::End => {
                    self.preview.last_view()?;
                    self.table_state.select_first();
                }
                KeyCode::Char('s') => {
                    let col = self.table_state.selected_column().unwrap_or(0);
                    self.preview.cycle_sort(col)?;
                }
                KeyCode::Char('/') => self.show_user_input = !self.show_user_input,
                _ => {}
            }
        }

        Ok(false)
    }
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
                && app.handle_input_key(key)?
            {
                return Ok(());
            }
        }
    })
}
