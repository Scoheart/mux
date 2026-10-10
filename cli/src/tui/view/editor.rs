//! Full-page catalog-entry editor. Navigate fields with ↑↓; Enter edits the
//! focused field (or toggles transport); Ctrl-S saves. Key hints live in the
//! footer, like every other screen.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;

use super::widgets::{self, Field};
use crate::tui::model::{Model, EDITOR_FIELDS};

pub fn render(model: &Model, f: &mut Frame, area: Rect) {
    let Some(ed) = model.editor.as_ref() else {
        return;
    };
    let title = if ed.original_key.is_none() {
        "新建 MCP"
    } else {
        "编辑 MCP"
    };

    let labels = ed.labels();
    let fields: Vec<Field> = labels
        .iter()
        .enumerate()
        .take(EDITOR_FIELDS)
        .map(|(i, label)| Field {
            label,
            value: ed.value(i),
            focused: i == ed.field,
            editing: i == ed.field && ed.editing,
            locked: (i == 0 && !ed.name_editable()) || (i == 3 && !ed.transport_editable()),
        })
        .collect();
    let mut lines = widgets::form_lines(&fields);

    let help = if ed.field == 3 && ed.transport_editable() {
        Some("Enter 切换 stdio ↔ http")
    } else if !ed.name_editable() && ed.field == 0 {
        Some("名称属于资产标识，编辑时不可修改")
    } else if !ed.transport_editable() && ed.field == 3 {
        Some("传输类型属于资产标识，编辑时不可修改")
    } else {
        None
    };
    if let Some(note) = widgets::form_note(ed.error.as_deref(), help) {
        lines.push(Line::from(""));
        lines.push(note);
    }

    let mut block = widgets::panel(title, true).padding(Padding::new(1, 2, 1, 0));
    if let Some(key) = &ed.original_key {
        block = block.title_top(widgets::corner_note(key.clone()));
    }
    f.render_widget(Paragraph::new(lines).block(block), area);
}
