//! Sources screen: the list of catalog sources with kind, count, and state.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, ListItem, Paragraph};
use ratatui::Frame;

use super::theme::render_list;
use crate::tui::model::Model;
use mux_core::sources::SourceView;

pub fn render(model: &Model, f: &mut Frame, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(area);

    let enabled: u32 = model
        .data
        .sources
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.server_count)
        .sum();
    let header = Line::from(vec![
        Span::from(format!("共 {} 个 server", enabled)).bold(),
        Span::from("（已启用来源）").dim(),
    ]);
    f.render_widget(Paragraph::new(header), rows[0]);

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().dim());
    if model.data.sources.is_empty() {
        let msg = "还没有来源。订阅远程配置、导入本地配置，或添加 Mux 精选。";
        f.render_widget(
            Paragraph::new(Line::from(Span::from(msg).dim())).block(block),
            rows[1],
        );
        return;
    }

    let items: Vec<ListItem> = model
        .data
        .sources
        .iter()
        .map(|s| ListItem::new(source_line(s)))
        .collect();
    render_list(f, rows[1], block, items, model.sources_ui.cursor, true);
}

fn source_line(s: &SourceView) -> Line<'static> {
    let badge = if s.managed {
        Span::from(" MUX 维护 ").black().on_yellow()
    } else if s.kind == "remote" {
        Span::from(" 订阅 ").black().on_cyan()
    } else {
        Span::from(" 本地 ").black().on_green()
    };
    let state = if s.enabled {
        Span::from("✓ 启用").green()
    } else {
        Span::from("✗ 停用").dim()
    };
    let mut spans = vec![
        Span::from(s.name.clone()).bold(),
        Span::from("  "),
        badge,
        Span::from(format!("  {} server  ", s.server_count)).dim(),
        state,
    ];
    if let Some(err) = &s.error {
        spans.push(Span::from(format!("  ⚠ {}", err)).red());
    }
    Line::from(spans)
}
