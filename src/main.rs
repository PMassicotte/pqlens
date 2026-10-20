mod filter;
mod keymaps;
mod reader;
mod sorter;
mod ui;

use crate::ui::render;
use reader::ParquetPreview;

use crossterm::event::{self, KeyCode};
use ratatui::widgets::TableState;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let parquet_file_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./data/sample-users.parquet".to_string());

    let mut preview = ParquetPreview::from_file(&parquet_file_path, 45)?;

    let mut table_state = TableState::default();

    table_state.select_first();
    table_state.select_first_column();

    let mut show_keymaps = false;

    let mut show_user_input = false;
    let mut user_input = String::new();

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| {
                render(
                    frame,
                    &mut table_state,
                    &preview,
                    show_keymaps,
                    show_user_input,
                    &user_input,
                );
            })?;
            if let Some(key) = event::read()?.as_key_press_event() {
                if show_user_input {
                    match key.code {
                        KeyCode::Char(c) => user_input.push(c),
                        KeyCode::Backspace => {
                            user_input.pop();
                        }
                        KeyCode::Esc => {
                            user_input = String::new();
                            show_user_input = false;
                        }
                        KeyCode::Enter => {
                            if user_input.is_empty() {
                                preview.clear_filter()?;
                            } else {
                                preview.set_filter(
                                    table_state.selected_column().unwrap_or(0),
                                    &user_input,
                                )?;
                            }

                            show_user_input = false;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('c') => {
                            preview.clear_filter()?;
                            preview.clear_sort();
                        }
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if show_keymaps {
                                show_keymaps = false;
                            } else {
                                return Ok(());
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => table_state.select_next(),
                        KeyCode::Char('k') | KeyCode::Up => table_state.select_previous(),
                        KeyCode::Char('l') | KeyCode::Right => table_state.select_next_column(),
                        KeyCode::Char('h') | KeyCode::Left => table_state.select_previous_column(),
                        KeyCode::Char('g') => table_state.select_first(),
                        KeyCode::Char('G') => table_state.select_last(),
                        KeyCode::Char('f') => {
                            if preview.is_filtered() {
                                preview.clear_filter()?;
                            } else {
                                let Some((row, col)) = table_state.selected_cell() else {
                                    continue;
                                };

                                preview.filter_by_cell(row, col)?;
                            }
                        }
                        KeyCode::Char('?') => show_keymaps = !show_keymaps,
                        KeyCode::PageDown => {
                            preview.next_view()?;
                            table_state.select_first();
                        }
                        KeyCode::PageUp => {
                            preview.previous_view()?;
                            table_state.select_first();
                        }
                        KeyCode::Home => {
                            preview.first_view()?;
                            table_state.select_first();
                        }
                        KeyCode::End => {
                            preview.last_view()?;
                            table_state.select_first();
                        }
                        KeyCode::Char('s') => {
                            let col = table_state.selected_column().unwrap_or(0);
                            preview.cycle_sort(col)?;
                        }
                        KeyCode::Char('/') => show_user_input = !show_user_input,
                        _ => {}
                    }
                }
            }
        }
    })
}
