use crate::keymaps::render_keymaps;
use crate::reader::ParquetPreview;
use crate::sorter::SortDir;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Cell, Row, Table, TableState};

/// Render the UI with a table.
pub fn render(
    frame: &mut Frame,
    table_state: &mut TableState,
    preview: &ParquetPreview,
    show_keymaps: bool,
) {
    let layout = Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).spacing(1);

    let [top, main] = frame.area().layout(&layout);

    let title = Text::from(vec![
        Line::from(" Press 'q' to quit, arrow keys to navigate, 's' to sort, '?' for keymaps"),
        Line::from(
            format!(
                " Batch {}/{}: {} rows in total",
                preview.current_batch() + 1,
                preview.num_batches(),
                preview.total_rows()
            )
            .bold()
            .fg(Color::Green),
        ),
    ]);

    frame.render_widget(title.centered(), top);

    render_table(frame, main, table_state, preview);

    if show_keymaps {
        render_keymaps(frame);
    }
}

/// Render a table with some rows and columns.
fn render_table(
    frame: &mut Frame,
    area: Rect,
    table_state: &mut TableState,
    preview: &ParquetPreview,
) {
    let header = Row::new(
        preview
            .header()
            .iter()
            .enumerate()
            .zip(preview.column_types().iter())
            .map(|((i, title), column_type)| {
                let title = match preview.sort_state() {
                    Some(s) if s.col() == i => {
                        let arrow = match s.dir() {
                            SortDir::Asc => " ▲",
                            SortDir::Desc => " ▼",
                        };

                        format!("{title}{arrow}")
                    }
                    _ => title.clone(),
                };

                Cell::from(vec![
                    Line::from(Span::styled(title, Style::default().fg(Color::Red).bold())),
                    Line::from(Span::styled(
                        column_type.as_str(),
                        Style::default().fg(Color::LightYellow).italic(),
                    )),
                ])
            }),
    )
    .height(2)
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
        .column_highlight_style(Style::new().fg(Color::Blue))
        .cell_highlight_style(Style::new().reversed().yellow());

    frame.render_stateful_widget(table, area, table_state);
}
