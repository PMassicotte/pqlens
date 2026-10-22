use crate::app::App;
use crate::keymaps::render_keymaps;
use crate::reader::ParquetPreview;
use crate::sorter::SortDir;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table, TableState};

/// Render the UI with a table.
pub fn render(frame: &mut Frame, app: &mut App) {
    // Frame contains the whole drawable area with .area(), split that into the table and the status
    // information areas
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(3),
    ]);

    let [header_area, table_area, info_area] = frame.area().layout(&layout);

    let title = Text::from(vec![
        Line::from(" Press 'q' or 'Esc' to quit, arrow keys to navigate and '?' for keymaps"),
        Line::from(
            format!(
                " Batch {}/{}: {} of {} rows in total",
                app.preview.current_view() + 1,
                app.preview.num_views(),
                app.preview.visible_rows(),
                app.preview.total_rows()
            )
            .bold()
            .fg(Color::Green),
        ),
    ]);

    frame.render_widget(title.centered(), header_area);
    render_table(frame, table_area, &mut app.table_state, &app.preview);
    render_status(frame, info_area, app);

    if app.show_keymaps {
        render_keymaps(frame);
    }

    if app.show_user_input {
        let colname = app.preview.header()[app.table_state.selected_column().unwrap_or(0)].clone();
        render_input_text(frame, &colname, &app.user_input);
    }
}

fn render_input_text(frame: &mut Frame, colname: &str, user_input: &str) {
    let textbox = Paragraph::new(user_input)
        .block(Block::bordered().title(format!("Search term for '{colname}' (Esc to cancel)")));

    let [_, bottom] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(frame.area());

    let bottom_right = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(bottom)[1];

    frame.set_cursor_position(Position {
        x: (bottom_right.x + 1 + user_input.chars().count() as u16)
            .min(bottom_right.x + bottom_right.width - 1),
        y: bottom_right.y + 1,
    });

    frame.render_widget(Clear, bottom_right);
    frame.render_widget(textbox, bottom_right);
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
        .map(|row| Row::new(row.iter().map(std::string::String::as_str)))
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

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let selected_column_idx = app.table_state.selected_column().unwrap_or(0);
    let selected_column_name = app.preview.header()[selected_column_idx].clone();
    let selected_column_type = app.preview.column_types()[selected_column_idx].clone();

    let selected_column_info = format!("{selected_column_name} ({selected_column_type})");

    let min_max = app.preview.column_min_max(selected_column_idx).map_or_else(
        |_| selected_column_info.clone(),
        |(min, max)| format!("{selected_column_info}: min: {min} / max: {max}"),
    );

    frame.render_widget(
        Paragraph::new(min_max).block(Block::bordered().title("Selected column info")),
        area,
    );
}
