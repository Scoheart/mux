//! Registry screen: search box, origin filter, and the catalog list.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, ListItem, Paragraph};
use ratatui::Frame;

use super::theme::{origin_tag, render_list, transport_pill};
use crate::tui::model::{Model, OriginFilter};

pub fn render(model: &Model, f: &mut Frame, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(1), // search
        Constraint::Length(1), // filter
        Constraint::Min(0),    // list
    ])
    .split(area);

    render_search(model, f, rows[0]);
    render_filter(model, f, rows[1]);
    render_catalog(model, f, rows[2]);
}

fn render_search(model: &Model, f: &mut Frame, area: Rect) {
    let ui = &model.registry_ui;
    let mut spans = vec![Span::from("🔎 ")];
    if ui.query.is_empty() && !ui.searching {
        spans.push(Span::from("按 / 搜索").dim());
    } else {
        spans.push(Span::from(ui.query.clone()));
        if ui.searching {
            spans.push(Span::from("▏").cyan()); // cursor caret
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_filter(model: &Model, f: &mut Frame, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for filt in OriginFilter::ALL {
        let label = format!(" {} ", filt.label());
        if filt == model.registry_ui.filter {
            spans.push(Span::styled(label, Style::new().black().on_cyan()));
        } else {
            spans.push(Span::from(label).dim());
        }
    }
    let count = model.filtered_registry().len();
    spans.push(Span::from(format!("   {} 个", count)).dim());
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_catalog(model: &Model, f: &mut Frame, area: Rect) {
    let entries = model.filtered_registry();
    let block = Block::default().borders(Borders::TOP).border_style(Style::new().dim());

    if entries.is_empty() {
        let msg = if model.data.registry.is_empty() {
            "目录为空。到「来源」订阅或导入，或用 mux import 探索现有配置。"
        } else {
            "没有匹配的条目。"
        };
        f.render_widget(Paragraph::new(Line::from(Span::from(msg).dim())).block(block), area);
        return;
    }

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let usage = model.usage_count(e);
            let mut spans = vec![
                Span::from(e.name.clone()).green().bold(),
                Span::from("  "),
                transport_pill(e),
                Span::from("  "),
                origin_tag(e),
            ];
            if model.data.custom_keys.contains(&e.key()) {
                spans.push(Span::from("  ✎ 自定义").yellow());
            }
            if usage > 0 {
                spans.push(Span::from(format!("  {} 用", usage)).blue());
            }
            if !e.description.is_empty() {
                spans.push(Span::from(format!("  — {}", e.description)).dim());
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    render_list(f, area, block, items, model.registry_ui.cursor, !model.registry_ui.searching);
}
