//! The only place the model changes. Pure and synchronous: mutate the model,
//! return side effects to run. No I/O here — that's what makes it unit-testable.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::effect::Effect;
use super::message::Msg;
use super::model::{
    AddMcpState, AgentPane, ConfirmState, Data, InstallWizard, Model, Modal, Screen,
};

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
        Msg::Mutated { label, result } => match result {
            Ok(()) => {
                model.status = Some(format!("✓ {}", label));
                return vec![Effect::LoadAll];
            }
            Err(e) => model.status = Some(format!("✗ {}：{}", label, e)),
        },
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
        return modal_key(model, k);
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
        _ => {
            return match model.screen {
                Screen::Registry => registry_key(model, k),
                Screen::Sources => sources_key(model, k),
                Screen::Agents => agents_key(model, k),
            }
        }
    }
    vec![]
}

fn registry_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
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
        KeyCode::Char('i') => open_install_wizard(model),
        _ => {}
    }
    vec![]
}

/// Open the multi-agent install target picker for the entry under the cursor.
fn open_install_wizard(model: &mut Model) {
    let (server, transport) = {
        let entries = model.filtered_registry();
        let Some(e) = entries.get(model.registry_ui.cursor) else {
            return;
        };
        (e.name.clone(), e.transport().to_string())
    };
    let n = model.installable_agents().len();
    if n == 0 {
        model.status = Some("没有可安装的 agent（缺少全局配置路径）".into());
        return;
    }
    model.modal = Some(Modal::Install(InstallWizard {
        server,
        transport,
        cursor: 0,
        selected: vec![false; n],
    }));
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

fn sources_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
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
    vec![]
}

fn agents_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
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
                KeyCode::Char('a') => open_add_mcp(model),
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
                KeyCode::Char('a') => open_add_mcp(model),
                KeyCode::Char(' ') => return toggle_installed(model),
                KeyCode::Char('d') => open_delete_confirm(model),
                _ => {}
            }
        }
    }
    vec![]
}

/// The installed row currently under the cursor, as (server, transport, agent).
fn selected_installed(model: &Model) -> Option<(String, String, String)> {
    let row = model
        .installed_for_selected_agent()
        .get(model.agents_ui.installed_cursor)
        .copied()?;
    Some((row.name.clone(), row.transport.clone(), row.agent.clone()))
}

/// Space on an installed row: enable a disabled server, or disable an active one.
fn toggle_installed(model: &mut Model) -> Vec<Effect> {
    let Some(row) = model
        .installed_for_selected_agent()
        .get(model.agents_ui.installed_cursor)
        .copied()
    else {
        return vec![];
    };
    let (server, transport, agent) = (row.name.clone(), row.transport.clone(), row.agent.clone());
    if row.enabled {
        vec![Effect::Disable { server, transport, agent }]
    } else {
        vec![Effect::Enable { server, transport, agent }]
    }
}

/// `d` on an installed row: gate a hard delete behind a Confirm modal.
fn open_delete_confirm(model: &mut Model) {
    let Some((server, transport, agent)) = selected_installed(model) else {
        return;
    };
    model.modal = Some(Modal::Confirm(ConfirmState {
        prompt: format!("从 {} 删除 {}？此操作会写回配置文件（有备份）。", agent, server),
        effect: Effect::Delete { server, transport, agent },
    }));
}

/// `a`: open the "add MCP to this agent" search popover for the selected agent.
fn open_add_mcp(model: &mut Model) {
    let Some(agent) = model.data.agents.get(model.agents_ui.agent_cursor) else {
        return;
    };
    if !agent.has_global {
        model.status = Some("该 agent 无全局路径，无法安装".into());
        return;
    }
    model.modal = Some(Modal::AddMcp(AddMcpState {
        agent: agent.id.clone(),
        query: String::new(),
        cursor: 0,
    }));
}

/// Route input to the open modal. The modal is taken out of the model; handlers
/// that keep it open put it back, ones that close it (cancel / commit) don't.
fn modal_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
    let Some(modal) = model.modal.take() else {
        return vec![];
    };
    match modal {
        Modal::Detail { .. } | Modal::Help => {
            if !matches!(k.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                model.modal = Some(modal); // ignore other keys, stay open
            }
            vec![]
        }
        Modal::Install(w) => install_key(model, w, k),
        Modal::AddMcp(st) => add_mcp_key(model, st, k),
        Modal::Confirm(c) => match k.code {
            KeyCode::Char('y') | KeyCode::Enter => vec![c.effect], // commit + close
            _ => vec![],                                           // n / Esc / other → cancel
        },
    }
}

fn install_key(model: &mut Model, mut w: InstallWizard, k: KeyEvent) -> Vec<Effect> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let agents: Vec<String> = model.installable_agents().iter().map(|a| a.id.clone()).collect();
    match k.code {
        KeyCode::Esc => return vec![], // cancel (already taken out)
        KeyCode::Up | KeyCode::Char('k') => w.cursor = w.cursor.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => w.cursor = clamp(w.cursor + 1, agents.len()),
        KeyCode::Char(' ') => {
            if let Some(s) = w.selected.get_mut(w.cursor) {
                *s = !*s;
            }
        }
        KeyCode::Char('a') if ctrl => {
            let all_on = w.selected.iter().all(|s| *s);
            for s in w.selected.iter_mut() {
                *s = !all_on;
            }
        }
        KeyCode::Enter => {
            let chosen: Vec<String> = agents
                .iter()
                .zip(&w.selected)
                .filter(|(_, s)| **s)
                .map(|(a, _)| a.clone())
                .collect();
            if chosen.is_empty() {
                model.status = Some("请至少选择一个 agent".into());
            } else {
                return vec![Effect::Install {
                    server: w.server,
                    transport: w.transport,
                    agents: chosen,
                }]; // commit + close
            }
        }
        _ => {}
    }
    model.modal = Some(Modal::Install(w));
    vec![]
}

fn add_mcp_key(model: &mut Model, mut st: AddMcpState, k: KeyEvent) -> Vec<Effect> {
    // The popover is a search field: printable keys type into the query; only
    // arrows navigate, Enter installs, Esc cancels.
    let matches: Vec<(String, String)> = model
        .addable_entries(&st.agent, &st.query)
        .iter()
        .map(|e| (e.name.clone(), e.transport().to_string()))
        .collect();
    match k.code {
        KeyCode::Esc => return vec![], // cancel
        KeyCode::Up => st.cursor = st.cursor.saturating_sub(1),
        KeyCode::Down => st.cursor = clamp(st.cursor + 1, matches.len()),
        KeyCode::Backspace => {
            st.query.pop();
            st.cursor = 0;
        }
        KeyCode::Enter => {
            if let Some((server, transport)) = matches.get(st.cursor).cloned() {
                return vec![Effect::Install {
                    server,
                    transport,
                    agents: vec![st.agent],
                }]; // commit + close
            }
        }
        KeyCode::Char(c) => {
            st.query.push(c);
            st.cursor = 0;
        }
        _ => {}
    }
    model.modal = Some(Modal::AddMcp(st));
    vec![]
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

    fn agent(id: &str, has_global: bool) -> mux_core::agents::AgentInfo {
        mux_core::agents::AgentInfo {
            id: id.into(),
            format: "json".into(),
            key: "mcpServers".into(),
            has_global,
            has_project: false,
            enabled: true,
            global: has_global.then(|| "~/x.json".to_string()),
            project: None,
        }
    }

    fn loaded_full(entries: Vec<RegistryEntry>, agents: Vec<mux_core::agents::AgentInfo>) -> Msg {
        Msg::Loaded(Box::new(LoadedData {
            registry: entries,
            custom_keys: vec![],
            sources: vec![],
            agents,
            installed: vec![],
        }))
    }

    #[test]
    fn install_wizard_select_and_apply_emits_install() {
        let mut m = Model::new();
        update(&mut m, loaded_full(vec![entry("git", "manual")], vec![agent("claude", true), agent("cursor", true)]));
        update(&mut m, key('i')); // open wizard
        assert!(matches!(m.modal, Some(Modal::Install(_))));
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()))); // select claude
        let eff = update(&mut m, code(KeyCode::Enter));
        assert_eq!(eff.len(), 1);
        assert!(matches!(&eff[0], Effect::Install { server, agents, .. } if server == "git" && agents == &vec!["claude".to_string()]));
        assert!(m.modal.is_none()); // closed on commit
    }

    #[test]
    fn install_wizard_empty_selection_refuses() {
        let mut m = Model::new();
        update(&mut m, loaded_full(vec![entry("git", "manual")], vec![agent("claude", true)]));
        update(&mut m, key('i'));
        let eff = update(&mut m, code(KeyCode::Enter)); // nothing selected
        assert!(eff.is_empty());
        assert!(matches!(m.modal, Some(Modal::Install(_)))); // stays open
    }

    #[test]
    fn install_needs_an_installable_agent() {
        let mut m = Model::new();
        update(&mut m, loaded_full(vec![entry("git", "manual")], vec![agent("nopath", false)]));
        update(&mut m, key('i'));
        assert!(m.modal.is_none()); // no wizard — nothing installable
        assert!(m.status.is_some());
    }

    #[test]
    fn confirm_yes_runs_effect_no_cancels() {
        use crate::tui::model::ConfirmState;
        let mut m = Model::new();
        m.modal = Some(Modal::Confirm(ConfirmState {
            prompt: "x".into(),
            effect: Effect::Delete { server: "git".into(), transport: "stdio".into(), agent: "claude".into() },
        }));
        let eff = update(&mut m, key('y'));
        assert_eq!(eff.len(), 1);
        assert!(matches!(&eff[0], Effect::Delete { .. }));
        assert!(m.modal.is_none());

        // 'n' cancels without an effect
        let mut m2 = Model::new();
        m2.modal = Some(Modal::Confirm(ConfirmState {
            prompt: "x".into(),
            effect: Effect::Delete { server: "git".into(), transport: "stdio".into(), agent: "claude".into() },
        }));
        let eff2 = update(&mut m2, key('n'));
        assert!(eff2.is_empty());
        assert!(m2.modal.is_none());
    }

    #[test]
    fn mutated_ok_sets_status_and_reloads() {
        let mut m = Model::new();
        let eff = update(&mut m, Msg::Mutated { label: "安装 git".into(), result: Ok(()) });
        assert_eq!(eff.len(), 1); // reload
        assert!(m.status.as_deref().unwrap().contains("✓"));
    }
}
