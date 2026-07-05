//! Full-page catalog-entry editor. Navigate fields with ↑↓; Enter edits the
//! focused field (or toggles transport); Ctrl-S saves.

use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::model::{EditorState, Model, EDITOR_FIELDS};

pub fn render(model: &Model, f: &mut Frame, area: Rect) {
    let Some(ed) = model.editor.as_ref() else {
        return;
    };
    let title = if ed.original_key.is_none() {
        " 新建 MCP "
    } else {
        " 编辑 MCP "
    };

    let labels = ed.labels();
    let mut lines: Vec<Line> = Vec::new();
    for i in 0..EDITOR_FIELDS {
        lines.push(field_line(ed, i, labels[i]));
    }
    lines.push(Line::from(""));
    if let Some(err) = &ed.error {
        lines.push(Line::from(Span::from(format!("✗ {}", err)).red()));
    } else if ed.field == 3 {
        lines.push(Line::from(Span::from("Enter 切换 stdio ↔ http").dim()));
    } else if !ed.name_editable() && ed.field == 0 {
        lines.push(Line::from(Span::from("名称不可修改（非自定义条目）").dim()));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().cyan())
        .title(Span::from(title).bold())
        .title_bottom(Line::from(
            Span::from(" ↑↓ 字段 · Enter 编辑/切换 · Ctrl-S 保存 · r 恢复默认 · Esc 取消 ").dim(),
        ));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn field_line(ed: &EditorState, i: usize, label: &str) -> Line<'static> {
    let focused = i == ed.field;
    let caret = if focused {
        Span::from("› ").cyan().bold()
    } else {
        Span::from("  ")
    };
    let label_span = Span::from(format!("{:<14}", label)).dim();

    let raw = ed.value(i);
    let editing = focused && ed.editing;
    let value = if raw.is_empty() && !editing {
        Span::from("—").dim()
    } else if i == 0 && !ed.name_editable() {
        Span::from(raw).dim()
    } else if focused {
        Span::from(raw).white()
    } else {
        Span::from(raw)
    };

    let mut spans = vec![caret, label_span, value];
    if editing {
        spans.push(Span::from("▏").cyan());
    }
    Line::from(spans)
}
