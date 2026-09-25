use ratatui::prelude::Stylize;
use ratatui::{
    Frame,
    text::Span,
    widgets::{Block, Clear, Row, Table},
};

use ratatui::layout::Constraint;

const KEYMAPS: &[(&str, &str)] = &[
    ("q / Esc", "Quit"),
    ("j/k, ↓/↑", "Move row"),
    ("h/l, ←/→", "Move column"),
    ("g / G", "First / last row"),
    ("PgUp / PgDn", "Previous / next batch"),
    ("Home / End", "First / last batch"),
    ("s", "Sort by column"),
    ("?", "Toggle this help"),
];

pub fn render_keymaps(frame: &mut Frame) {
    let rows = KEYMAPS
        .iter()
        .map(|(k, d)| Row::new([Span::from(*k).bold().yellow(), Span::from(*d)]));

    let table = Table::new(rows, [Constraint::Length(12), Constraint::Fill(1)])
        .block(Block::bordered().title(" Keymaps "));

    let area = frame.area().centered(
        Constraint::Length(50),
        Constraint::Length(KEYMAPS.len() as u16 + 2), // +2 for borders
    );
    frame.render_widget(Clear, area);
    frame.render_widget(table, area);
}
