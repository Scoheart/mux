//! Agents screen: an agent list on the left, the selected agent's config path +
//! installed MCPs on the right.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, ListItem, Paragraph};
use ratatui::Frame;

use super::theme::render_list;
use crate::tui::model::{AgentPane, Model};

pub fn render(model: &Model, f: &mut Frame, area: Rect) {
    let cols =
        Layout::horizontal([Constraint::Percentage(34), Constraint::Percentage(66)]).split(area);
    render_agent_list(model, f, cols[0]);
    render_installed(model, f, cols[1]);
}

fn render_agent_list(model: &Model, f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::new().dim())
        .title(Span::from(" Agents ").bold());

    if model.data.agents.is_empty() {
        f.render_widget(
            Paragraph::new(Span::from("无 agent").dim()).block(block),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = model
        .data
        .agents
        .iter()
        .map(|a| {
            let state = if !a.has_global {
                Span::from("· ").dim()
            } else if a.enabled {
                Span::from("● ").green()
            } else {
                Span::from("○ ").dim()
            };
            let mut spans = vec![state, Span::from(a.name.clone())];
            if !a.has_global {
                spans.push(Span::from("  目录").dim());
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let focused = model.agents_ui.pane == AgentPane::List;
    render_list(f, area, block, items, model.agents_ui.agent_cursor, focused);
}

fn render_installed(model: &Model, f: &mut Frame, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).split(area);

    let Some(agent) = model.data.agents.get(model.agents_ui.agent_cursor) else {
        return;
    };
    let path = agent
        .global
        .clone()
        .unwrap_or_else(|| "未设置全局路径".into());
    let head = vec![
        Line::from(vec![
            Span::from(agent.id.clone()).bold(),
            Span::from(format!("  · {}", agent.format)).dim(),
        ]),
        Line::from(Span::from(path).dim()),
    ];
    f.render_widget(Paragraph::new(head), rows[0]);

    let installed = model.installed_for_selected_agent();
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().dim())
        .title(Span::from(format!(" 已安装 MCP（{}）", installed.len())));

    if installed.is_empty() {
        f.render_widget(
            Paragraph::new(Span::from("暂无安装").dim()).block(block),
            rows[1],
        );
        return;
    }

    let items: Vec<ListItem> = installed
        .iter()
        .map(|i| {
            let state = if i.enabled {
                Span::from("✓ ").green()
            } else {
                Span::from("✗ ").dim()
            };
            let name = if i.enabled {
                Span::from(i.name.clone())
            } else {
                Span::from(i.name.clone()).dim()
            };
            let mut spans = vec![
                state,
                name,
                Span::from(format!("  [{}]", i.transport)).dim(),
            ];
            if i.customized {
                spans.push(Span::from("  ✎ 已改").yellow());
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let focused = model.agents_ui.pane == AgentPane::Installed;
    render_list(
        f,
        rows[1],
        block,
        items,
        model.agents_ui.installed_cursor,
        focused,
    );
}
