//! Shared rendering helpers: the color language, provenance/transport tags, and
//! a stateless scrolling-list widget (selection derived from a plain cursor, so
//! the model stays a `usize` and `view` never holds widget state).

use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Block, List, ListItem, ListState};
use ratatui::Frame;

use crate::tui::model::bucket_of;
use mux_core::types::RegistryEntry;

/// `[stdio]` / `[http]` transport pill.
pub fn transport_pill(entry: &RegistryEntry) -> Span<'static> {
    match entry.transport() {
        "stdio" => Span::from(" stdio ").black().on_blue(),
        _ => Span::from(" http ").black().on_magenta(),
    }
}

/// Provenance tag reflecting the entry's origin bucket.
pub fn origin_tag(entry: &RegistryEntry) -> Span<'static> {
    match bucket_of(entry) {
        "remote" => Span::from("订阅").cyan(),
        "local" => Span::from("本地").green(),
        "manual" => Span::from("手动").yellow(),
        _ => Span::from("探索").dim(),
    }
}

/// Render a scrolling list whose selection is `cursor`. A fresh `ListState` is
/// built each frame so ratatui recomputes the scroll offset to keep the cursor
/// visible; `focused` brightens the selected row.
pub fn render_list(
    f: &mut Frame,
    area: Rect,
    block: Block,
    items: Vec<ListItem>,
    cursor: usize,
    focused: bool,
) {
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(cursor.min(items.len() - 1)));
    }
    let hl = if focused {
        Style::new().bold().on_dark_gray()
    } else {
        Style::new().bold()
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol("› ")
        .highlight_style(hl);
    f.render_stateful_widget(list, area, &mut state);
}
