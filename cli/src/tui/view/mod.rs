//! Pure rendering: read the model, draw widgets. Never mutates. Persistent chrome
//! (tab bar + footer) wraps a per-screen body; modals overlay everything.

mod agents;
mod editor;
mod modal;
mod registry;
mod sources;
mod theme;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::model::{Model, Screen};

pub fn view(model: &Model, f: &mut Frame) {
    let rows = Layout::vertical([
        Constraint::Length(2), // tab bar
        Constraint::Min(0),    // body
        Constraint::Length(1), // footer / status
    ])
    .split(f.area());

    render_tabs(model, f, rows[0]);
    render_body(model, f, rows[1]);
    render_footer(model, f, rows[2]);

    if model.modal.is_some() {
        modal::render(model, f);
    }
}

fn render_tabs(model: &Model, f: &mut Frame, area: Rect) {
    let mut spans: Vec<Span> = vec![Span::from("MUX").bold().magenta(), Span::from("   ")];
    for (i, s) in Screen::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, s.title());
        if *s == model.screen {
            spans.push(Span::styled(label, Style::new().black().on_cyan().bold()));
        } else {
            spans.push(Span::from(label).dim());
        }
        spans.push(Span::from(" "));
    }
    let block = Block::default().borders(Borders::BOTTOM).border_style(Style::new().dim());
    f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn render_body(model: &Model, f: &mut Frame, area: Rect) {
    if model.editor.is_some() {
        editor::render(model, f, area);
        return;
    }
    if model.loading {
        f.render_widget(Paragraph::new(Span::from("加载中…").dim()), area);
        return;
    }
    match model.screen {
        Screen::Registry => registry::render(model, f, area),
        Screen::Sources => sources::render(model, f, area),
        Screen::Agents => agents::render(model, f, area),
    }
}

fn render_footer(model: &Model, f: &mut Frame, area: Rect) {
    let line = if let Some(status) = &model.status {
        Line::from(Span::from(status.clone()).yellow())
    } else {
        Line::from(Span::from(footer_hint(model)).dim())
    };
    f.render_widget(Paragraph::new(line), area);
}

/// Context-sensitive footer, so the UI documents itself.
fn footer_hint(model: &Model) -> &'static str {
    if model.editor.is_some() {
        return "↑↓ 字段 · Enter 编辑/切换 · Ctrl-S 保存 · r 恢复默认 · Esc 取消";
    }
    if model.registry_ui.searching {
        return "输入以搜索 · Enter/Esc 结束 · Backspace 删除";
    }
    match model.screen {
        Screen::Registry => "↑↓ 移动 · / 搜索 · ←→ 过滤 · i 安装 · S 同步 · n 新建 · e 编辑 · p 粘贴 · Enter 详情 · q 退出",
        Screen::Sources => "↑↓ 移动 · Space 启停 · r 刷新 · d 删除 · s 订阅 · l 导入 · o 官方 · q 退出",
        Screen::Agents => "↑↓ 移动 · →/Enter 进入 · a 添加MCP · n 新建 · e 编辑 · Space 启停 · d 删除 · q 退出",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::message::LoadedData;
    use crate::tui::update::update;
    use crate::tui::message::Msg;
    use mux_core::types::{RegistryConfig, RegistryEntry, RegistryOrigin, StdioConfig};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn entry(name: &str, kind: &str) -> RegistryEntry {
        RegistryEntry {
            name: name.into(),
            description: String::new(),
            tags: vec![],
            config: RegistryConfig {
                stdio: Some(StdioConfig { command: "npx".into(), args: None, env: None }),
                http: None,
            },
            origin: Some(RegistryOrigin { kind: kind.into(), agent: None, scope: None, source: None }),
        }
    }

    /// Flatten the buffer, dropping whitespace so CJK wide-glyph continuation
    /// cells don't split needles.
    fn render(model: &Model) -> String {
        let mut terminal = Terminal::new(TestBackend::new(90, 16)).unwrap();
        terminal.draw(|f| view(model, f)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .flat_map(|c| c.symbol().chars())
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    fn loaded(model: &mut Model, entries: Vec<RegistryEntry>) {
        update(
            model,
            Msg::Loaded(Box::new(LoadedData {
                registry: entries,
                custom_keys: vec![],
                sources: vec![],
                agents: vec![],
                installed: vec![],
            })),
        );
    }

    #[test]
    fn renders_chrome_and_loading() {
        let text = render(&Model::new());
        assert!(text.contains("MUX"));
        assert!(text.contains("Registry"));
        assert!(text.contains("Agents"));
        assert!(text.contains("加载中"));
    }

    #[test]
    fn renders_registry_entries_after_load() {
        let mut m = Model::new();
        loaded(&mut m, vec![entry("filesystem", "manual"), entry("wiki", "remote")]);
        let text = render(&m);
        assert!(text.contains("filesystem"));
        assert!(text.contains("wiki"));
        assert!(!text.contains("加载中"));
    }

    #[test]
    fn empty_catalog_shows_hint() {
        let mut m = Model::new();
        loaded(&mut m, vec![]);
        let text = render(&m);
        assert!(text.contains("目录为空"));
    }
}
