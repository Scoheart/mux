//! The only place the model changes. Pure and synchronous: mutate the model,
//! return side effects to run. No I/O here — that's what makes it unit-testable.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::effect::Effect;
use super::message::Msg;
use super::model::{
    AddMcpState, AgentForm, AgentPane, ConfirmState, Data, EditorState, EditorTransport,
    InstallWizard, LocalForm, Model, Modal, PasteState, Screen, SubscribeForm, AGENT_FIELDS,
    EDITOR_FIELDS,
};
use mux_core::types::AgentDefinition;

/// The GitHub raw URL of the curated collection, offered as a remote subscribe.
const OFFICIAL_URL: &str = "https://raw.githubusercontent.com/Scoheart/mux/main/data/registry.json";

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
        Msg::Resynced { name, transport, result } => match result {
            Ok(outcome) => {
                if !outcome.skipped_customized.is_empty() {
                    // Some installs are hand-customized — offer to force-overwrite.
                    model.modal = Some(Modal::Confirm(ConfirmState {
                        prompt: format!(
                            "{} 个 agent 的配置被手改过（{}），强制覆盖为当前配置？",
                            outcome.skipped_customized.len(),
                            outcome.skipped_customized.join("、")
                        ),
                        effect: Effect::ResyncEntry { name, transport, force: true },
                    }));
                } else if outcome.synced.is_empty() {
                    model.status = Some(format!("没有需要同步的已安装 agent：{}", name));
                } else {
                    model.status = Some(format!("✓ 已同步 {} 到 {} 个 agent", name, outcome.synced.len()));
                }
                // Reflect any clean installs that were just re-stamped.
                return vec![Effect::LoadAll];
            }
            Err(e) => model.status = Some(format!("✗ 同步失败：{}", e)),
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
    // The full-page editor captures all input (including digits/q) while open.
    if model.editor.is_some() {
        return editor_key(model, k);
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
        KeyCode::Char('r') if ctrl => {
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
        KeyCode::Char('n') => model.editor = Some(EditorState::new_entry()),
        KeyCode::Char('e') => open_editor_edit(model),
        KeyCode::Char('p') => model.modal = Some(Modal::Paste(PasteState::default())),
        KeyCode::Char('S') => return resync_selected(model),
        _ => {}
    }
    vec![]
}

/// `S`: re-sync the selected entry's current config to its installed agents.
fn resync_selected(model: &mut Model) -> Vec<Effect> {
    let sel = {
        let entries = model.filtered_registry();
        entries
            .get(model.registry_ui.cursor)
            .map(|e| (e.name.clone(), e.transport().to_string()))
    };
    match sel {
        Some((name, transport)) => vec![Effect::ResyncEntry { name, transport, force: false }],
        None => vec![],
    }
}

/// Open the full-page editor pre-filled from the entry under the cursor.
fn open_editor_edit(model: &mut Model) {
    let ed = {
        let entries = model.filtered_registry();
        let Some(e) = entries.get(model.registry_ui.cursor) else {
            return;
        };
        let is_custom = model.data.custom_keys.contains(&e.key());
        EditorState::from_entry(e, is_custom)
    };
    model.editor = Some(ed);
}

fn editor_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(k.code, KeyCode::Char('c')) {
        model.should_quit = true;
        return vec![];
    }
    if ctrl && matches!(k.code, KeyCode::Char('s')) {
        return save_editor(model);
    }
    let Some(ed) = model.editor.as_mut() else {
        return vec![];
    };
    ed.error = None;

    if ed.editing {
        let field = ed.field;
        match k.code {
            KeyCode::Enter | KeyCode::Esc => ed.editing = false,
            KeyCode::Backspace => {
                if let Some(buf) = ed.field_mut(field) {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = ed.field_mut(field) {
                    buf.push(c);
                }
            }
            _ => {}
        }
        return vec![];
    }

    // Navigation mode.
    match k.code {
        KeyCode::Up | KeyCode::Char('k') => ed.field = ed.field.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => ed.field = clamp(ed.field + 1, EDITOR_FIELDS),
        KeyCode::Enter => {
            if ed.field == 3 {
                ed.transport = match ed.transport {
                    EditorTransport::Stdio => EditorTransport::Http,
                    EditorTransport::Http => EditorTransport::Stdio,
                };
            } else if ed.field_mut(ed.field).is_some() {
                ed.editing = true;
            }
        }
        KeyCode::Esc => model.editor = None, // cancel
        KeyCode::Char('r') => {
            // Revert a custom entry to its source default.
            if ed.is_custom {
                if let Some(old) = ed.original_key.clone() {
                    if let Some((name, transport)) = old.rsplit_once("::") {
                        let eff = Effect::RevertEntry {
                            name: name.to_string(),
                            transport: transport.to_string(),
                        };
                        model.editor = None;
                        return vec![eff];
                    }
                }
            }
        }
        _ => {}
    }
    vec![]
}

/// Validate the editor form, guard collisions, and emit the save effect.
fn save_editor(model: &mut Model) -> Vec<Effect> {
    let (result, original_key, is_custom) = {
        let Some(ed) = model.editor.as_ref() else {
            return vec![];
        };
        let origin = ed
            .original_key
            .as_ref()
            .and_then(|k| model.data.registry.iter().find(|e| &e.key() == k))
            .and_then(|e| e.origin.clone());
        (ed.to_entry(origin), ed.original_key.clone(), ed.is_custom)
    };
    match result {
        Err(msg) => {
            if let Some(ed) = model.editor.as_mut() {
                ed.error = Some(msg);
            }
            vec![]
        }
        Ok(entry) => {
            let new_key = entry.key();
            let collides = original_key.as_deref() != Some(new_key.as_str())
                && model.data.registry.iter().any(|e| e.key() == new_key);
            if collides {
                if let Some(ed) = model.editor.as_mut() {
                    ed.error = Some("已存在同名同传输的条目".into());
                }
                return vec![];
            }
            let delete_old = match &original_key {
                Some(old) if *old != new_key && is_custom => {
                    old.rsplit_once("::").map(|(n, t)| (n.to_string(), t.to_string()))
                }
                _ => None,
            };
            model.editor = None;
            vec![Effect::UpsertEntry { entry, delete_old }]
        }
    }
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
        KeyCode::Char(' ') | KeyCode::Enter => return toggle_source(model),
        KeyCode::Char('r') => return refresh_source(model),
        KeyCode::Char('d') => open_remove_source_confirm(model),
        KeyCode::Char('s') => model.modal = Some(Modal::Subscribe(SubscribeForm::default())),
        KeyCode::Char('l') => model.modal = Some(Modal::AddLocal(LocalForm::default())),
        KeyCode::Char('o') => {
            model.modal = Some(Modal::Subscribe(SubscribeForm {
                url: OFFICIAL_URL.into(),
                name: "官方精选合集".into(),
                field: 0,
            }))
        }
        _ => {}
    }
    vec![]
}

/// The source row under the cursor.
fn current_source(model: &Model) -> Option<&mux_core::sources::SourceView> {
    model.data.sources.get(model.sources_ui.cursor)
}

fn toggle_source(model: &mut Model) -> Vec<Effect> {
    let Some(s) = current_source(model) else {
        return vec![];
    };
    vec![Effect::SetSourceEnabled { id: s.id.clone(), on: !s.enabled }]
}

/// `r`: refresh a remote/local source, or re-scan for the managed discovered one.
fn refresh_source(model: &mut Model) -> Vec<Effect> {
    let Some(s) = current_source(model) else {
        return vec![];
    };
    if s.managed {
        if s.id == "discovered" {
            return vec![Effect::ImportDiscovered];
        }
        model.status = Some("手动来源无需刷新".into());
        return vec![];
    }
    vec![Effect::RefreshSource { id: s.id.clone() }]
}

fn open_remove_source_confirm(model: &mut Model) {
    let Some(s) = current_source(model) else {
        return;
    };
    if s.managed {
        model.status = Some("MUX 维护的来源不可删除".into());
        return;
    }
    let (id, name) = (s.id.clone(), s.name.clone());
    model.modal = Some(Modal::Confirm(ConfirmState {
        prompt: format!("删除来源「{}」？其缓存文件会一并删除。", name),
        effect: Effect::RemoveSource { id },
    }));
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
                KeyCode::Char(' ') => return toggle_agent_enabled(model),
                KeyCode::Char('e') => open_agent_edit(model),
                KeyCode::Char('n') => model.modal = Some(Modal::AddAgent(AgentForm::new_agent())),
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

/// Space on an agent row: flip its enabled flag (persisted via agents::put).
fn toggle_agent_enabled(model: &mut Model) -> Vec<Effect> {
    let Some(a) = model.data.agents.get(model.agents_ui.agent_cursor) else {
        return vec![];
    };
    let def = AgentDefinition {
        global: a.global.clone(),
        project: a.project.clone(),
        format: a.format.clone(),
        key: a.key.clone(),
        enabled: !a.enabled,
        builtin: None,
    };
    vec![Effect::PutAgent { id: a.id.clone(), def, overwrite: true }]
}

/// `e` on an agent row: open its config-path editor.
fn open_agent_edit(model: &mut Model) {
    let Some(a) = model.data.agents.get(model.agents_ui.agent_cursor) else {
        return;
    };
    model.modal = Some(Modal::AddAgent(AgentForm::from_agent(a)));
}

fn agent_form_key(model: &mut Model, mut form: AgentForm, k: KeyEvent) -> Vec<Effect> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(k.code, KeyCode::Char('s')) {
        let (id, def) = form.to_def();
        let err = if id.is_empty() {
            Some("ID 不能为空")
        } else if def.key.trim().is_empty() {
            Some("配置 key 不能为空")
        } else if def.global.is_none() && def.project.is_none() {
            Some("至少填写一个配置路径")
        } else {
            None
        };
        if let Some(e) = err {
            form.error = Some(e.into());
            model.modal = Some(Modal::AddAgent(form));
            return vec![];
        }
        let overwrite = form.is_edit;
        return vec![Effect::PutAgent { id, def, overwrite }]; // commit + close
    }
    form.error = None;

    if form.editing {
        let field = form.field;
        match k.code {
            KeyCode::Enter | KeyCode::Esc => form.editing = false,
            KeyCode::Backspace => {
                if let Some(buf) = form.field_mut(field) {
                    buf.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buf) = form.field_mut(field) {
                    buf.push(c);
                }
            }
            _ => {}
        }
        model.modal = Some(Modal::AddAgent(form));
        return vec![];
    }

    match k.code {
        KeyCode::Esc => return vec![], // cancel
        KeyCode::Up | KeyCode::Char('k') => form.field = form.field.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => form.field = clamp(form.field + 1, AGENT_FIELDS),
        KeyCode::Enter => {
            if form.field == 1 {
                form.format_toml = !form.format_toml;
            } else if form.field_mut(form.field).is_some() {
                form.editing = true;
            }
        }
        _ => {}
    }
    model.modal = Some(Modal::AddAgent(form));
    vec![]
}

/// Route input to the open modal. The modal is taken out of the model; handlers
/// that keep it open put it back, ones that close it (cancel / commit) don't.
fn modal_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
    let Some(modal) = model.modal.take() else {
        return vec![];
    };
    match modal {
        Modal::Detail { key } => {
            // `e` jumps straight to editing this entry.
            if matches!(k.code, KeyCode::Char('e')) {
                if let Some(entry) = model.data.registry.iter().find(|e| e.key() == key) {
                    let is_custom = model.data.custom_keys.contains(&key);
                    model.editor = Some(EditorState::from_entry(entry, is_custom));
                }
                return vec![];
            }
            if !matches!(k.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                model.modal = Some(Modal::Detail { key }); // ignore other keys, stay open
            }
            vec![]
        }
        Modal::Help => {
            if !matches!(k.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                model.modal = Some(Modal::Help);
            }
            vec![]
        }
        Modal::Install(w) => install_key(model, w, k),
        Modal::AddMcp(st) => add_mcp_key(model, st, k),
        Modal::Confirm(c) => match k.code {
            KeyCode::Char('y') | KeyCode::Enter => vec![c.effect], // commit + close
            _ => vec![],                                           // n / Esc / other → cancel
        },
        Modal::Paste(st) => paste_key(model, st, k),
        Modal::Subscribe(form) => subscribe_key(model, form, k),
        Modal::AddLocal(form) => local_key(model, form, k),
        Modal::AddAgent(form) => agent_form_key(model, form, k),
    }
}

/// Trim a form value to an `Option` (empty → None).
fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

fn subscribe_key(model: &mut Model, mut form: SubscribeForm, k: KeyEvent) -> Vec<Effect> {
    match k.code {
        KeyCode::Esc => return vec![], // cancel
        KeyCode::Tab | KeyCode::Down | KeyCode::Up => form.field = 1 - form.field,
        KeyCode::Enter => {
            if form.url.trim().is_empty() {
                model.status = Some("URL 不能为空".into());
            } else {
                return vec![Effect::Subscribe { url: form.url.clone(), name: opt(&form.name) }];
            }
        }
        KeyCode::Backspace => {
            let buf = if form.field == 0 { &mut form.url } else { &mut form.name };
            buf.pop();
        }
        KeyCode::Char(c) => {
            let buf = if form.field == 0 { &mut form.url } else { &mut form.name };
            buf.push(c);
        }
        _ => {}
    }
    model.modal = Some(Modal::Subscribe(form));
    vec![]
}

fn local_key(model: &mut Model, mut form: LocalForm, k: KeyEvent) -> Vec<Effect> {
    match k.code {
        KeyCode::Esc => return vec![], // cancel
        KeyCode::Tab | KeyCode::Down | KeyCode::Up => form.field = 1 - form.field,
        KeyCode::Enter => {
            if form.path.trim().is_empty() {
                model.status = Some("路径不能为空".into());
            } else {
                return vec![Effect::AddLocal { path: form.path.clone(), name: opt(&form.name) }];
            }
        }
        KeyCode::Backspace => {
            let buf = if form.field == 0 { &mut form.path } else { &mut form.name };
            buf.pop();
        }
        KeyCode::Char(c) => {
            let buf = if form.field == 0 { &mut form.path } else { &mut form.name };
            buf.push(c);
        }
        _ => {}
    }
    model.modal = Some(Modal::AddLocal(form));
    vec![]
}

fn paste_key(model: &mut Model, mut st: PasteState, k: KeyEvent) -> Vec<Effect> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Esc => return vec![], // cancel
        KeyCode::Char('s') if ctrl => {
            if st.text.trim().is_empty() {
                model.status = Some("粘贴内容为空".into());
            } else {
                return vec![Effect::ImportPaste(st.text)]; // commit + close
            }
        }
        KeyCode::Enter => st.text.push('\n'),
        KeyCode::Backspace => {
            st.text.pop();
        }
        KeyCode::Char(c) => st.text.push(c),
        _ => {}
    }
    model.modal = Some(Modal::Paste(st));
    vec![]
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

    fn ctrl(c: char) -> Msg {
        Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
    }
    fn typed(m: &mut Model, s: &str) {
        for c in s.chars() {
            update(m, key(c));
        }
    }

    #[test]
    fn new_entry_fill_and_save_emits_upsert() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![]));
        update(&mut m, key('n'));
        assert!(m.editor.is_some());
        update(&mut m, code(KeyCode::Enter)); // edit name
        typed(&mut m, "git");
        update(&mut m, code(KeyCode::Enter)); // commit name
        assert_eq!(m.editor.as_ref().unwrap().name, "git");
        for _ in 0..4 {
            update(&mut m, code(KeyCode::Down)); // to command field (index 4)
        }
        update(&mut m, code(KeyCode::Enter));
        typed(&mut m, "npx");
        update(&mut m, code(KeyCode::Enter));
        let eff = update(&mut m, ctrl('s'));
        assert_eq!(eff.len(), 1);
        assert!(matches!(&eff[0], Effect::UpsertEntry { entry, delete_old: None } if entry.name == "git"));
        assert!(m.editor.is_none());
    }

    #[test]
    fn save_with_empty_name_shows_error_and_stays() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![]));
        update(&mut m, key('n'));
        let eff = update(&mut m, ctrl('s'));
        assert!(eff.is_empty());
        assert!(m.editor.as_ref().unwrap().error.is_some());
    }

    #[test]
    fn transport_toggle_swaps_field_set() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![]));
        update(&mut m, key('n'));
        for _ in 0..3 {
            update(&mut m, code(KeyCode::Down)); // to transport field (index 3)
        }
        assert!(matches!(m.editor.as_ref().unwrap().transport, EditorTransport::Stdio));
        update(&mut m, code(KeyCode::Enter)); // toggle
        assert!(matches!(m.editor.as_ref().unwrap().transport, EditorTransport::Http));
    }

    #[test]
    fn paste_types_and_imports() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![]));
        update(&mut m, key('p'));
        assert!(matches!(m.modal, Some(Modal::Paste(_))));
        typed(&mut m, "{}");
        let eff = update(&mut m, ctrl('s'));
        assert_eq!(eff.len(), 1);
        assert!(matches!(&eff[0], Effect::ImportPaste(t) if t == "{}"));
    }

    fn source(id: &str, enabled: bool, managed: bool) -> mux_core::sources::SourceView {
        mux_core::sources::SourceView {
            id: id.into(),
            kind: "remote".into(),
            name: id.into(),
            url: Some("http://x".into()),
            path: None,
            format: "json".into(),
            enabled,
            added_at: None,
            synced_at: None,
            server_count: 1,
            error: None,
            managed,
        }
    }

    fn loaded_sources(srcs: Vec<mux_core::sources::SourceView>) -> Msg {
        Msg::Loaded(Box::new(LoadedData {
            registry: vec![],
            custom_keys: vec![],
            sources: srcs,
            agents: vec![],
            installed: vec![],
        }))
    }

    #[test]
    fn space_toggles_source_enabled() {
        let mut m = Model::new();
        m.screen = Screen::Sources;
        update(&mut m, loaded_sources(vec![source("s1", true, false)]));
        let eff = update(&mut m, key(' '));
        assert!(matches!(&eff[0], Effect::SetSourceEnabled { id, on: false } if id == "s1"));
    }

    #[test]
    fn managed_source_cannot_be_removed() {
        let mut m = Model::new();
        m.screen = Screen::Sources;
        update(&mut m, loaded_sources(vec![source("manual", true, true)]));
        update(&mut m, key('d'));
        assert!(m.modal.is_none()); // no confirm
        assert!(m.status.is_some());
    }

    #[test]
    fn subscribe_form_submits() {
        let mut m = Model::new();
        m.screen = Screen::Sources;
        update(&mut m, loaded_sources(vec![]));
        update(&mut m, key('s'));
        assert!(matches!(m.modal, Some(Modal::Subscribe(_))));
        typed(&mut m, "http://example.com/r.json");
        let eff = update(&mut m, code(KeyCode::Enter));
        assert!(matches!(&eff[0], Effect::Subscribe { url, .. } if url == "http://example.com/r.json"));
    }

    #[test]
    fn discovered_refresh_triggers_import() {
        let mut m = Model::new();
        m.screen = Screen::Sources;
        update(&mut m, loaded_sources(vec![source("discovered", true, true)]));
        let eff = update(&mut m, key('r'));
        assert!(matches!(&eff[0], Effect::ImportDiscovered));
    }

    #[test]
    fn space_toggles_agent_enabled() {
        let mut m = Model::new();
        m.screen = Screen::Agents;
        update(&mut m, loaded_full(vec![], vec![agent("claude", true)]));
        let eff = update(&mut m, key(' '));
        assert!(matches!(&eff[0], Effect::PutAgent { id, def, overwrite: true }
            if id == "claude" && !def.enabled));
    }

    #[test]
    fn new_agent_form_fill_and_save() {
        let mut m = Model::new();
        m.screen = Screen::Agents;
        update(&mut m, loaded_full(vec![], vec![agent("claude", true)]));
        update(&mut m, key('n'));
        assert!(matches!(m.modal, Some(Modal::AddAgent(_))));
        update(&mut m, code(KeyCode::Enter)); // edit id
        typed(&mut m, "myagent");
        update(&mut m, code(KeyCode::Enter));
        for _ in 0..3 {
            update(&mut m, code(KeyCode::Down)); // to global path (field 3)
        }
        update(&mut m, code(KeyCode::Enter));
        typed(&mut m, "~/x.json");
        update(&mut m, code(KeyCode::Enter));
        let eff = update(&mut m, ctrl('s'));
        assert!(matches!(&eff[0], Effect::PutAgent { id, overwrite: false, .. } if id == "myagent"));
        assert!(m.modal.is_none());
    }

    #[test]
    fn shift_s_resyncs_selected_entry() {
        let mut m = Model::new();
        update(&mut m, loaded(vec![entry("git", "manual")]));
        let eff = update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT)));
        assert!(matches!(&eff[0], Effect::ResyncEntry { name, force: false, .. } if name == "git"));
    }

    #[test]
    fn resynced_with_customized_opens_force_confirm() {
        let mut m = Model::new();
        let outcome = mux_core::ops::ResyncOutcome {
            synced: vec![],
            skipped_customized: vec!["qoder".into()],
        };
        update(
            &mut m,
            Msg::Resynced { name: "luma".into(), transport: "stdio".into(), result: Ok(outcome) },
        );
        match &m.modal {
            Some(Modal::Confirm(c)) => {
                assert!(matches!(&c.effect, Effect::ResyncEntry { force: true, name, .. } if name == "luma"));
            }
            _ => panic!("expected a force-confirm modal"),
        }
    }

    #[test]
    fn resynced_clean_only_sets_status_no_modal() {
        let mut m = Model::new();
        let outcome = mux_core::ops::ResyncOutcome {
            synced: vec!["claude-code".into()],
            skipped_customized: vec![],
        };
        let eff = update(
            &mut m,
            Msg::Resynced { name: "luma".into(), transport: "stdio".into(), result: Ok(outcome) },
        );
        assert!(m.modal.is_none());
        assert!(m.status.as_deref().unwrap().contains("已同步"));
        assert_eq!(eff.len(), 1); // reload
    }

    #[test]
    fn agent_form_requires_a_path() {
        let mut m = Model::new();
        m.screen = Screen::Agents;
        update(&mut m, loaded_full(vec![], vec![agent("claude", true)]));
        update(&mut m, key('n'));
        update(&mut m, code(KeyCode::Enter)); // edit id
        typed(&mut m, "x");
        update(&mut m, code(KeyCode::Enter));
        let eff = update(&mut m, ctrl('s')); // no path given
        assert!(eff.is_empty());
        if let Some(Modal::AddAgent(form)) = &m.modal {
            assert!(form.error.is_some());
        } else {
            panic!("form should stay open with an error");
        }
    }
}
