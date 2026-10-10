//! Shared components built on the theme tokens: panels, the scrolling list,
//! key hints, form rows, dialogs and empty states. Screens compose these
//! instead of styling raw widgets, so every surface looks and behaves alike.

use ratatui::layout::{Alignment, Constraint, Flex, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;

use super::theme::{self, glyph, Tone};

/// Display width of `s` in terminal cells (CJK counts as two).
pub fn width(s: &str) -> usize {
    Span::raw(s).width()
}

/// `s` padded with spaces to `cells` display cells, or truncated with `…`.
pub fn fit(s: &str, cells: usize) -> String {
    let w = width(s);
    if cells == 0 {
        return String::new();
    }
    if w <= cells {
        return format!("{s}{}", " ".repeat(cells - w));
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = width(ch.encode_utf8(&mut [0; 4]));
        if used + cw + 1 > cells {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out.push('…');
    used += 1;
    out.push_str(&" ".repeat(cells.saturating_sub(used)));
    out
}

/// A titled, rounded panel. The focused panel gets an accent border and title.
pub fn panel<'a>(title: impl Into<String>, focused: bool) -> Block<'a> {
    let title_style = if focused {
        theme::selected(true)
    } else {
        theme::strong()
    };
    Block::bordered()
        .border_type(theme::BORDER)
        .border_style(theme::border(focused))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title.into(), title_style),
            Span::raw(" "),
        ]))
}

/// A right-aligned, muted annotation for a panel's top border.
pub fn corner_note<'a>(note: impl Into<String>) -> Line<'a> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(note.into(), theme::muted()),
        Span::raw(" "),
    ])
    .right_aligned()
}

/// Render a scrolling list whose selection is `cursor`. A fresh `ListState`
/// each frame lets ratatui keep the cursor visible, so the model stays a plain
/// `usize`. The selection gutter is accent in the focused pane and a thin
/// muted bar otherwise (rows style their own selected label); a scrollbar
/// appears on the border only on overflow.
pub fn render_list(
    f: &mut Frame,
    area: Rect,
    block: Block,
    items: Vec<ListItem>,
    cursor: usize,
    focused: bool,
) {
    let inner = block.inner(area);
    let len = items.len();
    let rows: usize = items.iter().map(ListItem::height).sum();
    let mut state = ListState::default();
    if len > 0 {
        state.select(Some(cursor.min(len - 1)));
    }
    let gutter = if focused {
        Span::styled(glyph::GUTTER, theme::fg(Tone::Accent))
    } else {
        Span::styled(glyph::GUTTER_IDLE, theme::muted())
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol(Line::from(gutter))
        .highlight_spacing(HighlightSpacing::Always)
        .scroll_padding(1);
    f.render_stateful_widget(list, area, &mut state);

    if rows > inner.height as usize && area.height > 2 {
        let mut sb = ScrollbarState::new(len.saturating_sub(1)).position(cursor);
        let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some(glyph::SCROLL_TRACK))
            .track_style(theme::muted())
            .thumb_symbol(glyph::SCROLL_THUMB)
            .thumb_style(theme::border(focused));
        f.render_stateful_widget(
            bar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut sb,
        );
    }
}

/// `key label  key label …` with keys in accent and labels muted.
pub fn hints(pairs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(pairs.len() * 3);
    for (i, (k, label)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(theme::GAP));
        }
        spans.push(Span::styled(k.to_string(), theme::key()));
        spans.push(Span::styled(format!(" {label}"), theme::muted()));
    }
    spans
}

/// Key hints for a dialog's bottom border.
pub fn border_hints<'a>(pairs: &[(&str, &str)]) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(hints(pairs));
    spans.push(Span::raw(" "));
    Line::from(spans).right_aligned()
}

/// One row in a form.
pub struct Field<'a> {
    pub label: &'a str,
    pub value: String,
    pub focused: bool,
    pub editing: bool,
    /// Part of the asset identity (or otherwise fixed): shown but not editable.
    pub locked: bool,
}

/// Form rows with labels aligned on display width, a focus gutter, and an
/// accent caret while the value is being edited.
pub fn form_lines(fields: &[Field]) -> Vec<Line<'static>> {
    let label_w = fields.iter().map(|f| width(f.label)).max().unwrap_or(0) + 2;
    fields
        .iter()
        .map(|fl| {
            let gutter = if fl.focused {
                Span::styled(glyph::GUTTER, theme::fg(Tone::Accent))
            } else {
                Span::raw(glyph::NO_GUTTER)
            };
            let label_style = if fl.focused {
                theme::selected(true)
            } else {
                theme::muted()
            };
            let value = if fl.value.is_empty() && !fl.editing {
                Span::styled(glyph::EMPTY, theme::muted())
            } else if fl.locked {
                Span::styled(fl.value.clone(), theme::muted())
            } else if fl.focused {
                Span::styled(fl.value.clone(), theme::strong())
            } else {
                Span::styled(fl.value.clone(), theme::text())
            };
            let mut spans = vec![
                gutter,
                Span::styled(fit(fl.label, label_w), label_style),
                value,
            ];
            if fl.editing {
                spans.push(Span::styled(glyph::CARET, theme::fg(Tone::Accent)));
            }
            Line::from(spans)
        })
        .collect()
}

/// A muted helper line under a form, or the form's error in danger.
pub fn form_note(error: Option<&str>, help: Option<&str>) -> Option<Line<'static>> {
    if let Some(err) = error {
        return Some(Line::from(vec![
            Span::styled(format!("{} ", glyph::FAIL), theme::fg(Tone::Danger)),
            Span::styled(err.to_string(), theme::fg(Tone::Danger)),
        ]));
    }
    help.map(|h| Line::from(Span::styled(h.to_string(), theme::muted())))
}

/// A centered `w` × `h` rect, clamped to `area` minus a one-cell margin.
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2)).max(1);
    let h = h.min(area.height.saturating_sub(2)).max(1);
    let [row] = Layout::vertical([Constraint::Length(h)])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(w)])
        .flex(Flex::Center)
        .areas(row);
    cell
}

/// Dim everything already drawn, clear `area`, and draw a dialog frame in
/// `tone`. Returns the padded content area.
pub fn dialog(f: &mut Frame, area: Rect, title: &str, tone: Tone, keys: &[(&str, &str)]) -> Rect {
    let screen = f.area();
    f.buffer_mut()
        .set_style(screen, Style::new().add_modifier(Modifier::DIM));
    f.render_widget(Clear, area);
    let mut block = Block::bordered()
        .border_type(theme::BORDER)
        .border_style(theme::fg(tone))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title.to_string(),
                theme::fg(tone).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .padding(Padding::new(2, 2, 1, 0));
    if !keys.is_empty() {
        block = block.title_bottom(border_hints(keys));
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

/// A vertically centered empty/placeholder state: a strong title plus muted
/// detail lines.
pub fn empty_state(f: &mut Frame, area: Rect, title: &str, detail: &[&str]) {
    let mut lines = vec![Line::from(Span::styled(title.to_string(), theme::strong()))];
    for d in detail {
        lines.push(Line::from(Span::styled(d.to_string(), theme::muted())));
    }
    let h = (lines.len() as u16).min(area.height);
    let [mid] = Layout::vertical([Constraint::Length(h)])
        .flex(Flex::Center)
        .areas(area);
    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        mid,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_pads_by_display_width() {
        assert_eq!(fit("名称", 6), "名称  ");
        assert_eq!(fit("ab", 4), "ab  ");
        assert_eq!(width(&fit("参数（JSON 数组）", 20)), 20);
    }

    #[test]
    fn fit_truncates_with_ellipsis() {
        assert_eq!(fit("filesystem", 5), "file…");
        assert_eq!(width(&fit("配置文件地址", 5)), 5);
        assert_eq!(fit("x", 0), "");
    }

    #[test]
    fn form_labels_align_across_scripts() {
        let fields = [
            Field {
                label: "名称",
                value: "a".into(),
                focused: false,
                editing: false,
                locked: false,
            },
            Field {
                label: "URL",
                value: "b".into(),
                focused: true,
                editing: true,
                locked: false,
            },
        ];
        let lines = form_lines(&fields);
        let value_col = |l: &Line| l.spans[0].width() + l.spans[1].width();
        assert_eq!(value_col(&lines[0]), value_col(&lines[1]));
    }
}
