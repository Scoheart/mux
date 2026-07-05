//! The only place the model changes. Pure and synchronous: mutate the model,
//! return side effects to run. No I/O here — that's what makes it unit-testable.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::effect::Effect;
use super::message::Msg;
use super::model::{AgentPane, Data, Model, Modal, Screen};

pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Init => return vec![Effect::LoadAll],
        Msg::Tick => model.tick = model.tick.wrapping_add(1),
        Msg::Redraw => {}
        Msg::Loaded(d) => {
            model.loading = false;
            model.data = Data {
                registry: d.registry,
                custom_keys: d.custom_keys.into_iter().collect::<HashSet<_>>(),
                sources: d.sources,
                agents: d.agents,
                installed: d.installed,
            };
            clamp_cursors(model);
        }
        Msg::Error(e) => model.status = Some(e),
        Msg::Key(k) => return on_key(model, k),
    }
    vec![]
}

/// Keep every cursor within the bounds of the data it indexes.
fn clamp_cursors(model: &mut Model) {
    let reg = model.filtered_registry().len();
    model.registry_ui.cursor = clamp(model.registry_ui.cursor, reg);
    model.sources_ui.cursor = clamp(model.sources_ui.cursor, model.data.sources.len());
    model.agents_ui.agent_cursor = clamp(model.agents_ui.agent_cursor, model.data.agents.len());
    let inst = model.installed_for_selected_agent().len();
    model.agents_ui.installed_cursor = clamp(model.agents_ui.installed_cursor, inst);
}

/// Clamp `cursor` to `[0, len-1]`, or 0 when empty.
fn clamp(cursor: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        cursor.min(len - 1)
    }
}

fn on_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
    // A modal captures all input until dismissed.
    if model.modal.is_some() {
        if matches!(k.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
            model.modal = None;
        }
        return vec![];
    }

    // The registry search box captures text while focused.
    if model.screen == Screen::Registry && model.registry_ui.searching {
        return registry_search_key(model, k);
    }

    // Global navigation-context keys.
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Char('c') if ctrl => model.should_quit = true,
        KeyCode::Char('q') => model.should_quit = true,
        KeyCode::Char('?') => model.modal = Some(Modal::Help),
        KeyCode::Char('1') => model.screen = Screen::Registry,
        KeyCode::Char('2') => model.screen = Screen::Sources,
        KeyCode::Char('3') => model.screen = Screen::Agents,
        KeyCode::Tab => model.screen = model.screen.next(),
        KeyCode::BackTab => model.screen = model.screen.prev(),
        KeyCode::Char('r') => {
            model.loading = true;
            return vec![Effect::LoadAll];
        }
        _ => match model.screen {
            Screen::Registry => registry_key(model, k),
            Screen::Sources => sources_key(model, k),
            Screen::Agents => agents_key(model, k),
        },
    }
    vec![]
}

fn registry_key(model: &mut Model, k: KeyEvent) {
    let len = model.filtered_registry().len();
    match k.code {
        KeyCode::Up | KeyCode::Char('k') => {
            model.registry_ui.cursor = model.registry_ui.cursor.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            model.registry_ui.cursor = clamp(model.registry_ui.cursor + 1, len);
        }
        KeyCode::Char('/') => model.registry_ui.searching = true,
        KeyCode::Left | KeyCode::Char('[') => {
            model.registry_ui.filter = model.registry_ui.filter.prev();
            model.registry_ui.cursor = 0;
        }
        KeyCode::Right | KeyCode::Char(']') => {
            model.registry_ui.filter = model.registry_ui.filter.next();
            model.registry_ui.cursor = 0;
        }
        KeyCode::Enter => {
            if let Some(e) = model.filtered_registry().get(model.registry_ui.cursor) {
                let key = e.key();
                model.modal = Some(Modal::Detail { key });
            }
        }
        _ => {}
    }
}

fn registry_search_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
    match k.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Down => model.registry_ui.searching = false,
        KeyCode::Backspace => {
            model.registry_ui.query.pop();
            model.registry_ui.cursor = 0;
        }
        KeyCode::Char(c) => {
            model.registry_ui.query.push(c);
            model.registry_ui.cursor = 0;
        }
        _ => {}
    }
    vec![]
}

fn sources_key(model: &mut Model, k: KeyEvent) {
    let len = model.data.sources.len();
    match k.code {
        KeyCode::Up | KeyCode::Char('k') => {
            model.sources_ui.cursor = model.sources_ui.cursor.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            model.sources_ui.cursor = clamp(model.sources_ui.cursor + 1, len);
        }
        _ => {}
    }
}

fn agents_key(model: &mut Model, k: KeyEvent) {
    match model.agents_ui.pane {
        AgentPane::List => {
            let len = model.data.agents.len();
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    model.agents_ui.agent_cursor = model.agents_ui.agent_cursor.saturating_sub(1);
                    model.agents_ui.installed_cursor = 0;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    model.agents_ui.agent_cursor = clamp(model.agents_ui.agent_cursor + 1, len);
                    model.agents_ui.installed_cursor = 0;
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                    if !model.installed_for_selected_agent().is_empty() {
                        model.agents_ui.pane = AgentPane::Installed;
                    }
                }
                _ => {}
            }
        }
        AgentPane::Installed => {
            let len = model.installed_for_selected_agent().len();
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    model.agents_ui.installed_cursor =
                        model.agents_ui.installed_cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    model.agents_ui.installed_cursor = clamp(model.agents_ui.installed_cursor + 1, len);
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => {
                    model.agents_ui.pane = AgentPane::List;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::message::LoadedData;
    use mux_core::types::{RegistryConfig, RegistryEntry, RegistryOrigin, StdioConfig};

    fn key(c: char) -> Msg {
        Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
    }
    fn code(c: KeyCode) -> Msg {
        Msg::Key(KeyEvent::new(c, KeyModifiers::empty()))
    }

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

    fn loaded(entries: Vec<RegistryEntry>) -> Msg {
        Msg::Loaded(Box::new(LoadedData {
            registry: entries,
            custom_keys: vec![],
            sources: vec![],
            agents: vec![],
            installed: vec![],
        }))
    }

    #[test]
    fn init_requests_load() {
        let mut m = Model::new();
        assert_eq!(update(&mut m, Msg::Init).len(), 1);
    }

    #[test]
    fn loaded_fills_data_and_clears_loading() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![entry("git", "manual"), entry("fs", "remote")]));
        assert!(!m.loading);
        assert_eq!(m.data.registry.len(), 2);
    }

    #[test]
    fn digit_keys_switch_screens() {
        let mut m = Model::new();
        update(&mut m, key('2'));
        assert!(m.screen == Screen::Sources);
        update(&mut m, key('3'));
        assert!(m.screen == Screen::Agents);
    }

    #[test]
    fn slash_enters_search_and_types_into_query() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![entry("git", "manual")]));
        update(&mut m, key('/'));
        assert!(m.registry_ui.searching);
        update(&mut m, key('g'));
        update(&mut m, key('i'));
        assert_eq!(m.registry_ui.query, "gi");
        // digits type into the query (not screen-switch) while searching
        update(&mut m, key('1'));
        assert_eq!(m.registry_ui.query, "gi1");
        assert!(m.screen == Screen::Registry);
        update(&mut m, code(KeyCode::Esc));
        assert!(!m.registry_ui.searching);
    }

    #[test]
    fn origin_filter_narrows_registry() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![entry("git", "manual"), entry("fs", "remote")]));
        // filter → 订阅 (remote) shows only fs
        update(&mut m, code(KeyCode::Right));
        assert_eq!(m.filtered_registry().len(), 1);
        assert_eq!(m.filtered_registry()[0].name, "fs");
    }

    #[test]
    fn enter_opens_and_dismisses_detail_modal() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![entry("git", "manual")]));
        update(&mut m, code(KeyCode::Enter));
        assert!(matches!(m.modal, Some(Modal::Detail { .. })));
        update(&mut m, code(KeyCode::Esc));
        assert!(m.modal.is_none());
    }

    #[test]
    fn cursor_clamps_to_filtered_len() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![entry("a", "manual"), entry("b", "manual")]));
        update(&mut m, code(KeyCode::Down));
        update(&mut m, code(KeyCode::Down)); // past end
        assert_eq!(m.registry_ui.cursor, 1);
    }

    #[test]
    fn q_and_ctrl_c_quit() {
        let mut m = Model::new();
        update(&mut m, key('q'));
        assert!(m.should_quit);
        let mut m2 = Model::new();
        update(&mut m2, Msg::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(m2.should_quit);
    }
}
