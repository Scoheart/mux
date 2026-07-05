//! Overlay dialogs drawn on top of the current screen. Phase 2: read-only Detail
//! and the Help cheatsheet.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::model::{Modal, Model};

pub fn render(model: &Model, f: &mut Frame) {
    match model.modal.as_ref() {
        Some(Modal::Detail { key }) => render_detail(model, f, key),
        Some(Modal::Help) => render_help(f),
        None => {}
    }
}

/// A centered rect `pct_x` × `pct_y` percent of `area`.
fn centered(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let [h] = Layout::horizontal([Constraint::Percentage(pct_x)])
        .flex(Flex::Center)
        .areas(area);
    let [v] = Layout::vertical([Constraint::Percentage(pct_y)])
        .flex(Flex::Center)
        .areas(h);
    v
}

fn render_detail(model: &Model, f: &mut Frame, key: &str) {
    let Some(entry) = model.data.registry.iter().find(|e| e.key() == key) else {
        return;
    };
    let area = centered(f.area(), 70, 70);
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(Span::from(entry.name.clone()).green().bold()),
        Line::from(vec![
            Span::from(format!("传输：{}", entry.transport())).dim(),
            Span::from(format!("   标签：{}", entry.tags.join(", "))).dim(),
        ]),
    ];
    if !entry.description.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(entry.description.clone()));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::from("配置：").dim()));
    if let Ok(json) = serde_json::to_string_pretty(&entry.config) {
        for l in json.lines() {
            lines.push(Line::from(Span::from(l.to_string()).cyan()));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().cyan())
        .title(Span::from(" 详情 "))
        .title_bottom(Line::from(Span::from(" Esc 关闭 · y 复制 · e 编辑 ").dim()));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
}

fn render_help(f: &mut Frame) {
    let area = centered(f.area(), 60, 70);
    f.render_widget(Clear, area);
    let rows = [
        ("1 / 2 / 3", "切换 Registry / 来源 / Agents"),
        ("Tab / ⇧Tab", "循环切换屏幕"),
        ("↑ ↓ / k j", "移动光标"),
        ("/", "在 Registry 搜索（Esc 退出）"),
        ("← →", "切换来源过滤（Registry）"),
        ("Enter", "打开详情 / 进入面板"),
        ("r", "重新加载"),
        ("?", "帮助"),
        ("q / Ctrl-C", "退出"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::from(format!("{:<12}", k)).cyan().bold(),
                Span::from(*d),
            ])
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().cyan())
        .title(Span::from(" 快捷键 "))
        .title_bottom(Line::from(Span::from(" Esc 关闭 ").dim()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}
