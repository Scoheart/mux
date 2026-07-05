//! Pure rendering: read the model, draw widgets. Never mutates. Phase 1 draws
//! the persistent chrome (tab bar + footer) and a placeholder body; Phase 2
//! fills each screen in its own file under here.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::model::{Model, Screen};

pub fn view(model: &Model, f: &mut Frame) {
    let rows = Layout::vertical([
        Constraint::Length(3), // tab bar
        Constraint::Min(0),    // body
        Constraint::Length(1), // footer / status
    ])
    .split(f.area());

    render_tabs(model, f, rows[0]);
    render_body(model, f, rows[1]);
    render_footer(model, f, rows[2]);
}

fn render_tabs(model: &Model, f: &mut Frame, area: Rect) {
    let mut spans: Vec<Span> = vec![Span::from("MUX").bold().magenta(), Span::from("   ")];
    for (i, s) in Screen::ALL.iter().enumerate() {
        let n = i + 1;
        let label = format!(" {} {} ", n, s.title());
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
    let text = if model.loading {
        vec![Line::from(Span::from("加载中…").dim())]
    } else if let Some(c) = &model.counts {
        match model.screen {
            Screen::Registry => vec![
                Line::from(format!("{} 个 MCP 在目录中", c.registry)),
                Line::from(Span::from("（Phase 2 将在这里渲染目录列表）").dim()),
            ],
            Screen::Sources => vec![
                Line::from(format!("{} 个来源", c.sources)),
                Line::from(Span::from("（Phase 2 将在这里渲染来源卡片）").dim()),
            ],
            Screen::Agents => vec![
                Line::from(format!("{} 个 agent · {} 处已安装", c.agents, c.installed)),
                Line::from(Span::from("（Phase 2 将在这里渲染每-agent 安装管理）").dim()),
            ],
        }
    } else {
        vec![Line::from(Span::from("无数据").dim())]
    };
    let block = Block::default().borders(Borders::NONE);
    f.render_widget(Paragraph::new(text).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::Counts;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Flatten the rendered buffer into a whitespace-free string. Wide (CJK)
    /// glyphs render into two cells (the second a blank continuation), so we drop
    /// whitespace before substring-matching to keep needles contiguous.
    fn render(model: &Model) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        terminal.draw(|f| view(model, f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        buf.content()
            .iter()
            .flat_map(|c| c.symbol().chars())
            .filter(|c| !c.is_whitespace())
            .collect()
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
    fn renders_registry_counts_when_loaded() {
        let mut m = Model::new();
        m.loading = false;
        m.counts = Some(Counts { registry: 7, agents: 18, sources: 2, installed: 3 });
        let text = render(&m);
        assert!(text.contains("7个MCP"));
    }
}

fn render_footer(model: &Model, f: &mut Frame, area: Rect) {
    let line = if let Some(status) = &model.status {
        Line::from(Span::from(status.clone()).yellow())
    } else {
        Line::from(vec![
            Span::from("1/2/3 切换 · Tab 循环 · r 刷新 · q 退出").dim(),
        ])
    };
    f.render_widget(Paragraph::new(line), area);
}
