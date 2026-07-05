//! Overlay dialogs drawn on top of the current screen. Phase 2: read-only Detail
//! and the Help cheatsheet.

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::model::{AddMcpState, ConfirmState, InstallWizard, Modal, Model, PasteState};

pub fn render(model: &Model, f: &mut Frame) {
    match model.modal.as_ref() {
        Some(Modal::Detail { key }) => render_detail(model, f, key),
        Some(Modal::Help) => render_help(f),
        Some(Modal::Install(w)) => render_install(model, f, w),
        Some(Modal::AddMcp(st)) => render_add_mcp(model, f, st),
        Some(Modal::Confirm(c)) => render_confirm(f, c),
        Some(Modal::Paste(st)) => render_paste(f, st),
        Some(Modal::Subscribe(form)) => {
            render_form(f, " 订阅 URL ", ["配置文件 URL", "名称（可选）"], [&form.url, &form.name], form.field)
        }
        Some(Modal::AddLocal(form)) => {
            render_form(f, " 导入本地文件 ", ["文件路径", "名称（可选）"], [&form.path, &form.name], form.field)
        }
        None => {}
    }
}

/// A small two-field form (Subscribe / AddLocal): Tab switches field, Enter submits.
fn render_form(f: &mut Frame, title: &str, labels: [&str; 2], values: [&str; 2], field: usize) {
    let area = centered(f.area(), 66, 30);
    f.render_widget(Clear, area);
    let mut lines = Vec::new();
    for i in 0..2 {
        let caret = if i == field {
            Span::from("› ").cyan().bold()
        } else {
            Span::from("  ")
        };
        let val = if values[i].is_empty() {
            Span::from("—").dim()
        } else if i == field {
            Span::from(values[i].to_string()).white()
        } else {
            Span::from(values[i].to_string())
        };
        let mut spans = vec![caret, Span::from(format!("{:<16}", labels[i])).dim(), val];
        if i == field {
            spans.push(Span::from("▏").cyan());
        }
        lines.push(Line::from(spans));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().cyan())
        .title(Span::from(title).bold())
        .title_bottom(Line::from(Span::from(" Tab 切换 · Enter 提交 · Esc 取消 ").dim()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_paste(f: &mut Frame, st: &PasteState) {
    let area = centered(f.area(), 75, 75);
    f.render_widget(Clear, area);
    let mut lines: Vec<Line> = if st.text.is_empty() {
        vec![Line::from(Span::from("在此粘贴 mcpServers JSON / TOML …").dim())]
    } else {
        st.text.lines().map(|l| Line::from(l.to_string())).collect()
    };
    lines.push(Line::from(Span::from("▏").cyan()));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().green())
        .title(Span::from(" 粘贴配置 "))
        .title_bottom(Line::from(Span::from(" Ctrl-S 识别并添加 · Esc 取消 ").dim()));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
}

fn render_install(model: &Model, f: &mut Frame, w: &InstallWizard) {
    let area = centered(f.area(), 60, 70);
    f.render_widget(Clear, area);
    let agents = model.installable_agents();
    let mut lines = vec![
        Line::from(vec![
            Span::from("安装 "),
            Span::from(w.server.clone()).green().bold(),
            Span::from(format!("  [{}]", w.transport)).dim(),
        ]),
        Line::from(Span::from("选择要安装到的 agent（全局）：").dim()),
        Line::from(""),
    ];
    for (i, a) in agents.iter().enumerate() {
        let checked = w.selected.get(i).copied().unwrap_or(false);
        let cbox = if checked {
            Span::from("◉ ").green()
        } else {
            Span::from("○ ").dim()
        };
        let caret = if i == w.cursor {
            Span::from("› ").cyan().bold()
        } else {
            Span::from("  ")
        };
        lines.push(Line::from(vec![caret, cbox, Span::from(a.id.clone())]));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().cyan())
        .title(Span::from(" 安装到 agent "))
        .title_bottom(Line::from(
            Span::from(" Space 选择 · Ctrl-A 全选 · Enter 应用 · Esc 取消 ").dim(),
        ));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_add_mcp(model: &Model, f: &mut Frame, st: &AddMcpState) {
    let area = centered(f.area(), 70, 70);
    f.render_widget(Clear, area);
    let entries = model.addable_entries(&st.agent, &st.query);
    let mut lines = vec![
        Line::from(vec![
            Span::from("🔎 "),
            Span::from(st.query.clone()),
            Span::from("▏").cyan(),
        ]),
        Line::from(""),
    ];
    if entries.is_empty() {
        lines.push(Line::from(Span::from("无可添加的条目").dim()));
    }
    for (i, e) in entries.iter().enumerate() {
        let caret = if i == st.cursor {
            Span::from("› ").cyan().bold()
        } else {
            Span::from("  ")
        };
        lines.push(Line::from(vec![
            caret,
            Span::from(e.name.clone()).green(),
            Span::from(format!("  [{}]", e.transport())).dim(),
        ]));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().green())
        .title(Span::from(format!(" 添加 MCP → {} ", st.agent)))
        .title_bottom(Line::from(
            Span::from(" 输入搜索 · ↑↓ 选择 · Enter 安装 · Esc 取消 ").dim(),
        ));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_confirm(f: &mut Frame, c: &ConfirmState) {
    let area = centered(f.area(), 60, 30);
    f.render_widget(Clear, area);
    let lines = vec![
        Line::from(c.prompt.clone()),
        Line::from(""),
        Line::from(vec![
            Span::from(" y 确认 ").black().on_red().bold(),
            Span::from("    "),
            Span::from("n / Esc 取消").dim(),
        ]),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().red())
        .title(Span::from(" 确认 "));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
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
        ("Ctrl-R", "重新加载全部"),
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
