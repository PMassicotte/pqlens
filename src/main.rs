pub mod reader;
use reader::ParquetPreview;

// ratatui table widgets, eventually
// https://ratatui.rs/examples/widgets/table/

use color_eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Row, Table, TableState};

#[derive(Clone, Copy, PartialEq)]
enum SortDir {
    Asc,
    Desc,
}

#[derive(Clone, Copy)]
struct SortState {
    col: usize,
    dir: SortDir,
}

impl SortState {
    fn next(current: Option<SortState>, col: usize) -> Option<SortState> {
        match current {
            Some(s) if s.col == col && s.dir == SortDir::Asc => Some(SortState {
                col,
                dir: SortDir::Desc,
            }),
            Some(s) if s.col == col && s.dir == SortDir::Desc => None,
            _ => Some(SortState {
                col,
                dir: SortDir::Asc,
            }),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut preview = ParquetPreview::from_file("./data/sample-users.parquet", 1)?;

    println!("{:?}", preview.header());

    for row in preview.rows() {
        println!("{:?}", row);
    }

    color_eyre::install()?;

    let mut table_state = TableState::default();

    table_state.select_first();
    table_state.select_first_column();

    let mut sort: Option<SortState> = None;

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| render(frame, &mut table_state, &preview, sort))?;
            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => table_state.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => table_state.select_previous(),
                    KeyCode::Char('l') | KeyCode::Right => table_state.select_next_column(),
                    KeyCode::Char('h') | KeyCode::Left => table_state.select_previous_column(),
                    KeyCode::Char('g') => table_state.select_first(),
                    KeyCode::Char('G') => table_state.select_last(),
                    KeyCode::Char('s') => {
                        let col = table_state.selected_column().unwrap_or(0);
                        sort = SortState::next(sort, col);
                        match sort {
                            Some(s) => preview.sort_by(s.col, s.dir == SortDir::Desc)?,
                            None => preview.clear_sort(),
                        }
                    }
                    _ => {}
                }
            }
        }
    })
}

/// Render the UI with a table.
fn render(
    frame: &mut Frame,
    table_state: &mut TableState,
    preview: &ParquetPreview,
    sort: Option<SortState>,
) {
    let layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let [top, main] = frame.area().layout(&layout);

    let title = Line::from_iter([
        Span::from("Table Widget").bold(),
        Span::from(" (Press 'q' to quit, arrow keys to navigate, 's' to sort)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_table(frame, main, table_state, preview, sort);
}

/// Render a table with some rows and columns.
fn render_table(
    frame: &mut Frame,
    area: Rect,
    table_state: &mut TableState,
    preview: &ParquetPreview,
    sort: Option<SortState>,
) {
    let header = Row::new(
        preview
            .header()
            .iter()
            .enumerate()
            .map(|(i, title)| match sort {
                Some(s) if s.col == i => {
                    let arrow = if s.dir == SortDir::Asc { "▲" } else { "▼" };
                    format!("{title} {arrow}")
                }
                _ => title.clone(),
            }),
    )
    .style(Style::new().bold())
    .bottom_margin(1);

    let rows: Vec<Row> = preview
        .rows()
        .map(|row| Row::new(row.iter().map(|r| r.as_str())))
        .collect();

    let widths: Vec<Constraint> = preview
        .header()
        .iter()
        .map(|title| Constraint::Min(title.len() as u16 + 2))
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .style(Color::White)
        .row_highlight_style(Style::new().on_black().bold())
        .column_highlight_style(Color::Gray)
        .cell_highlight_style(Style::new().reversed().yellow());

    frame.render_stateful_widget(table, area, table_state);
}
