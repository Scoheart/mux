//! The whole application state. Owned by the event loop; mutated only by
//! `update`; read only by `view`.

use std::collections::HashSet;
use std::time::Duration;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use mux_core::agents::AgentInfo;
use mux_core::ops::InstalledMcp;
use mux_core::sources::SourceView;
use mux_core::types::{HttpConfig, RegistryConfig, RegistryEntry, StdioConfig};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Registry,
    Sources,
    Agents,
}

impl Screen {
    pub fn title(self) -> &'static str {
        match self {
            Screen::Registry => "Registry",
            Screen::Sources => "来源",
            Screen::Agents => "Agents",
        }
    }

    pub const ALL: [Screen; 3] = [Screen::Registry, Screen::Sources, Screen::Agents];

    pub fn next(self) -> Screen {
        match self {
            Screen::Registry => Screen::Sources,
            Screen::Sources => Screen::Agents,
            Screen::Agents => Screen::Registry,
        }
    }

    pub fn prev(self) -> Screen {
        match self {
            Screen::Registry => Screen::Agents,
            Screen::Sources => Screen::Registry,
            Screen::Agents => Screen::Sources,
        }
    }
}

/// The authoritative caches, filled by `Effect::LoadAll` and re-read after any
/// mutation. Rendering derives everything from these.
#[derive(Default)]
pub struct Data {
    pub registry: Vec<RegistryEntry>,
    pub custom_keys: HashSet<String>,
    pub sources: Vec<SourceView>,
    pub agents: Vec<AgentInfo>,
    pub installed: Vec<InstalledMcp>,
}

/// Origin/provenance filter on the Registry screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OriginFilter {
    All,
    Remote,
    Local,
    Manual,
    Discovered,
}

impl OriginFilter {
    pub const ALL: [OriginFilter; 5] = [
        OriginFilter::All,
        OriginFilter::Remote,
        OriginFilter::Local,
        OriginFilter::Manual,
        OriginFilter::Discovered,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OriginFilter::All => "全部",
            OriginFilter::Remote => "订阅",
            OriginFilter::Local => "本地",
            OriginFilter::Manual => "手动",
            OriginFilter::Discovered => "探索",
        }
    }

    /// The `bucket` value this filter matches (None = All, matches everything).
    fn bucket(self) -> Option<&'static str> {
        match self {
            OriginFilter::All => None,
            OriginFilter::Remote => Some("remote"),
            OriginFilter::Local => Some("local"),
            OriginFilter::Manual => Some("manual"),
            OriginFilter::Discovered => Some("discovered"),
        }
    }

    pub fn next(self) -> OriginFilter {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> OriginFilter {
        let i = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// The provenance bucket of a catalog entry (drives the filter + the origin tag).
pub fn bucket_of(entry: &RegistryEntry) -> &'static str {
    match entry.origin.as_ref().map(|o| o.kind.as_str()) {
        Some("remote") => "remote",
        Some("local") => "local",
        Some("manual") => "manual",
        _ => "discovered",
    }
}

pub struct RegistryUi {
    pub query: String,
    pub filter: OriginFilter,
    pub cursor: usize,
    /// True while the search box has focus (keys type into `query`).
    pub searching: bool,
}

impl Default for RegistryUi {
    fn default() -> Self {
        Self {
            query: String::new(),
            filter: OriginFilter::All,
            cursor: 0,
            searching: false,
        }
    }
}

#[derive(Default)]
pub struct SourcesUi {
    pub cursor: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgentPane {
    List,
    Installed,
}

pub struct AgentsUi {
    pub agent_cursor: usize,
    pub installed_cursor: usize,
    pub pane: AgentPane,
}

impl Default for AgentsUi {
    fn default() -> Self {
        Self {
            agent_cursor: 0,
            installed_cursor: 0,
            pane: AgentPane::List,
        }
    }
}

/// Multi-agent target picker for installing a catalog entry (global scope in v1).
pub struct InstallWizard {
    pub server: String,
    pub transport: String,
    pub cursor: usize,
    /// Parallel to `Model::installable_agents()`; which targets are checked.
    pub selected: Vec<bool>,
}

/// The "add MCP to this agent" search popover on the Agents screen.
pub struct AddMcpState {
    pub agent: String,
    pub query: String,
    pub cursor: usize,
}

/// A destructive-action gate. `effect` fires on confirm.
pub struct ConfirmState {
    pub prompt: String,
    pub effect: super::effect::Effect,
}

/// Paste-a-config dialog: a single text buffer of JSON/TOML.
#[derive(Default)]
pub struct PasteState {
    pub text: String,
}

/// Two-field form (url + optional name) for subscribing to a remote source.
#[derive(Default)]
pub struct SubscribeForm {
    pub url: String,
    pub name: String,
    pub field: usize, // 0 = url, 1 = name
}

/// Two-field form (path + optional name) for importing a local source file.
#[derive(Default)]
pub struct LocalForm {
    pub path: String,
    pub name: String,
    pub field: usize, // 0 = path, 1 = name
}

pub const AGENT_FIELDS: usize = 4;

/// Create/edit an agent definition. Field 1 (format) toggles json↔toml; the id
/// is locked when editing. Navigate-vs-edit like the catalog editor.
pub struct AgentForm {
    pub is_edit: bool,
    pub id: String,
    pub format_toml: bool,
    pub key: String,
    pub global: String,
    /// Retained when editing an older definition; not exposed in the form.
    pub legacy_project: Option<String>,
    pub field: usize,
    pub editing: bool,
    pub error: Option<String>,
}

impl AgentForm {
    pub fn new_agent() -> Self {
        Self {
            is_edit: false,
            id: String::new(),
            format_toml: false,
            key: "mcpServers".into(),
            global: String::new(),
            legacy_project: None,
            field: 0,
            editing: false,
            error: None,
        }
    }

    pub fn from_agent(a: &AgentInfo) -> Self {
        Self {
            is_edit: true,
            id: a.id.clone(),
            format_toml: a.format == "toml",
            key: a.key.clone(),
            global: a.global.clone().unwrap_or_default(),
            legacy_project: a.project.clone(),
            field: 3, // land on the global path — the usual thing to edit
            editing: false,
            error: None,
        }
    }

    pub fn labels(&self) -> [&'static str; AGENT_FIELDS] {
        ["ID", "格式", "配置 key", "全局路径"]
    }

    pub fn value(&self, i: usize) -> String {
        match i {
            0 => self.id.clone(),
            1 => if self.format_toml { "toml" } else { "json" }.into(),
            2 => self.key.clone(),
            3 => self.global.clone(),
            _ => String::new(),
        }
    }

    pub fn id_editable(&self) -> bool {
        !self.is_edit
    }

    pub fn field_mut(&mut self, i: usize) -> Option<&mut String> {
        match i {
            0 if self.id_editable() => Some(&mut self.id),
            2 => Some(&mut self.key),
            3 => Some(&mut self.global),
            _ => None, // id (locked) or format (toggle)
        }
    }

    /// Build (id, AgentDefinition). Core's `agents::put` does the real validation
    /// (id/key/global path non-empty); this just assembles the fields.
    pub fn to_def(&self) -> (String, mux_core::types::AgentDefinition) {
        let opt = |s: &str| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };
        (
            self.id.trim().to_string(),
            mux_core::types::AgentDefinition {
                global: opt(&self.global),
                project: self.legacy_project.clone(),
                format: if self.format_toml {
                    "toml".into()
                } else {
                    "json".into()
                },
                key: self.key.trim().to_string(),
                enabled: true,
                builtin: None,
            },
        )
    }
}

/// Overlay dialogs.
pub enum Modal {
    /// Read-only catalog-entry detail, keyed by `name::transport`.
    Detail {
        key: String,
    },
    Help,
    Install(InstallWizard),
    AddMcp(AddMcpState),
    Confirm(ConfirmState),
    Paste(PasteState),
    Subscribe(SubscribeForm),
    AddLocal(LocalForm),
    AddAgent(AgentForm),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorTransport {
    Stdio,
    Http,
}

/// The full-page catalog-entry editor (create or edit). Multi-value fields are
/// single-line and comma-separated (`args`, `tags`) or `KEY=val,` (`env`,
/// `headers`), mirroring the old `mux add` form. Field 3 (transport) toggles
/// instead of taking text; the name is only editable when new or custom.
pub struct EditorState {
    pub original_key: Option<String>,
    pub is_custom: bool,
    pub name: String,
    pub description: String,
    pub tags: String,
    pub transport: EditorTransport,
    pub command: String,
    pub args: String,
    pub env: String,
    pub http_type: String,
    pub url: String,
    pub headers: String,
    pub repo: String,
    pub field: usize,
    pub editing: bool,
    pub error: Option<String>,
}

/// Number of fields (field 3 swaps meaning by transport; field 7 = repo, common).
pub const EDITOR_FIELDS: usize = 8;

impl EditorState {
    /// A blank editor for a new entry.
    pub fn new_entry() -> Self {
        Self {
            original_key: None,
            is_custom: true,
            name: String::new(),
            description: String::new(),
            tags: String::new(),
            transport: EditorTransport::Stdio,
            command: String::new(),
            args: String::new(),
            env: String::new(),
            http_type: "http".into(),
            url: String::new(),
            headers: String::new(),
            repo: String::new(),
            field: 0,
            editing: false,
            error: None,
        }
    }

    /// An editor pre-filled from an existing entry.
    pub fn from_entry(entry: &RegistryEntry, is_custom: bool) -> Self {
        let mut s = Self::new_entry();
        s.original_key = Some(entry.key());
        s.is_custom = is_custom;
        s.name = entry.name.clone();
        s.description = entry.description.clone();
        s.tags = entry.tags.join(", ");
        if let Some(stdio) = &entry.config.stdio {
            s.transport = EditorTransport::Stdio;
            s.command = stdio.command.clone();
            s.args = stdio.args.clone().unwrap_or_default().join(", ");
            s.env = kv_to_string(stdio.env.as_ref());
        } else if let Some(http) = &entry.config.http {
            s.transport = EditorTransport::Http;
            s.http_type = http.kind.clone();
            s.url = http.url.clone();
            s.headers = kv_to_string(http.headers.as_ref());
        }
        s.repo = entry.repo.clone().unwrap_or_default();
        s
    }

    pub fn name_editable(&self) -> bool {
        self.original_key.is_none() || self.is_custom
    }

    pub fn labels(&self) -> [&'static str; EDITOR_FIELDS] {
        match self.transport {
            EditorTransport::Stdio => [
                "名称",
                "描述",
                "标签（逗号）",
                "传输",
                "命令",
                "参数（逗号）",
                "环境（KEY=val,）",
                "仓库 URL",
            ],
            EditorTransport::Http => [
                "名称",
                "描述",
                "标签（逗号）",
                "传输",
                "类型",
                "URL",
                "请求头（KEY=val,）",
                "仓库 URL",
            ],
        }
    }

    /// The display value of field `i`.
    pub fn value(&self, i: usize) -> String {
        match (i, self.transport) {
            (0, _) => self.name.clone(),
            (1, _) => self.description.clone(),
            (2, _) => self.tags.clone(),
            (3, EditorTransport::Stdio) => "stdio".into(),
            (3, EditorTransport::Http) => "http / sse".into(),
            (4, EditorTransport::Stdio) => self.command.clone(),
            (5, EditorTransport::Stdio) => self.args.clone(),
            (6, EditorTransport::Stdio) => self.env.clone(),
            (4, EditorTransport::Http) => self.http_type.clone(),
            (5, EditorTransport::Http) => self.url.clone(),
            (6, EditorTransport::Http) => self.headers.clone(),
            (7, _) => self.repo.clone(),
            _ => String::new(),
        }
    }

    /// Mutable text buffer for field `i`, or `None` for the transport toggle
    /// (field 3) and the name when it isn't editable.
    pub fn field_mut(&mut self, i: usize) -> Option<&mut String> {
        match (i, self.transport) {
            (0, _) if self.name_editable() => Some(&mut self.name),
            (0, _) => None,
            (1, _) => Some(&mut self.description),
            (2, _) => Some(&mut self.tags),
            (3, _) => None,
            (4, EditorTransport::Stdio) => Some(&mut self.command),
            (5, EditorTransport::Stdio) => Some(&mut self.args),
            (6, EditorTransport::Stdio) => Some(&mut self.env),
            (4, EditorTransport::Http) => Some(&mut self.http_type),
            (5, EditorTransport::Http) => Some(&mut self.url),
            (6, EditorTransport::Http) => Some(&mut self.headers),
            (7, _) => Some(&mut self.repo),
            _ => None,
        }
    }

    /// Validate the form and build a `RegistryEntry`, preserving `origin`.
    pub fn to_entry(
        &self,
        origin: Option<mux_core::types::RegistryOrigin>,
    ) -> Result<RegistryEntry, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("名称不能为空".into());
        }
        let tags = split_commas(&self.tags);
        let config = match self.transport {
            EditorTransport::Stdio => {
                if self.command.trim().is_empty() {
                    return Err("stdio 需要填写命令".into());
                }
                let args = split_commas(&self.args);
                RegistryConfig {
                    stdio: Some(StdioConfig {
                        command: self.command.trim().to_string(),
                        args: if args.is_empty() { None } else { Some(args) },
                        env: parse_kv(&self.env),
                        cwd: None,
                    }),
                    http: None,
                }
            }
            EditorTransport::Http => {
                if self.url.trim().is_empty() {
                    return Err("http 需要填写 URL".into());
                }
                let kind = self.http_type.trim();
                RegistryConfig {
                    stdio: None,
                    http: Some(HttpConfig {
                        kind: if kind.is_empty() {
                            "http".into()
                        } else {
                            kind.to_string()
                        },
                        url: self.url.trim().to_string(),
                        headers: parse_kv(&self.headers),
                    }),
                }
            }
        };
        let repo = self.repo.trim();
        Ok(RegistryEntry {
            name: name.to_string(),
            description: self.description.trim().to_string(),
            tags,
            config,
            origin,
            repo: if repo.is_empty() {
                None
            } else {
                Some(repo.to_string())
            },
        })
    }
}

fn split_commas(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse `KEY=val, KEY2=val2` into a map (None if empty).
fn parse_kv(raw: &str) -> Option<std::collections::HashMap<String, String>> {
    let map: std::collections::HashMap<String, String> = raw
        .split(',')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            let (k, v) = (k.trim(), v.trim());
            if k.is_empty() {
                None
            } else {
                Some((k.to_string(), v.to_string()))
            }
        })
        .collect();
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Serialize a map back to `KEY=val, KEY2=val2` (keys sorted for stability).
fn kv_to_string(map: Option<&std::collections::HashMap<String, String>>) -> String {
    let Some(map) = map else { return String::new() };
    let mut pairs: Vec<(&String, &String)> = map.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(", ")
}

pub struct Model {
    pub screen: Screen,
    pub should_quit: bool,
    pub tick: u64,
    pub loading: bool,
    pub data: Data,
    pub registry_ui: RegistryUi,
    pub sources_ui: SourcesUi,
    pub agents_ui: AgentsUi,
    pub modal: Option<Modal>,
    /// Full-page catalog editor; when `Some`, it replaces the screen body.
    pub editor: Option<EditorState>,
    pub status: Option<String>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            screen: Screen::Registry,
            should_quit: false,
            tick: 0,
            loading: true,
            data: Data::default(),
            registry_ui: RegistryUi::default(),
            sources_ui: SourcesUi::default(),
            agents_ui: AgentsUi::default(),
            modal: None,
            editor: None,
            status: None,
        }
    }

    pub fn tick_interval(&self) -> Duration {
        Duration::from_secs(3600)
    }

    /// The registry entries visible under the current origin filter + fuzzy
    /// query, sorted by name. Rendering and cursor math both go through this so
    /// they never disagree.
    pub fn filtered_registry(&self) -> Vec<&RegistryEntry> {
        let matcher = SkimMatcherV2::default();
        let q = self.registry_ui.query.trim();
        let want = self.registry_ui.filter.bucket();
        let mut out: Vec<&RegistryEntry> = self
            .data
            .registry
            .iter()
            .filter(|e| want.map_or(true, |b| bucket_of(e) == b))
            .filter(|e| {
                if q.is_empty() {
                    return true;
                }
                let hay = format!("{} {} {}", e.name, e.description, e.tags.join(" "));
                matcher.fuzzy_match(&hay, q).is_some()
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// How many distinct agents currently have this server active (enabled).
    pub fn usage_count(&self, entry: &RegistryEntry) -> usize {
        let transport = entry.transport();
        let mut agents: HashSet<&str> = HashSet::new();
        for i in &self.data.installed {
            if i.enabled && i.name == entry.name && i.transport == transport {
                agents.insert(&i.agent);
            }
        }
        agents.len()
    }

    /// The MCP rows installed (or remembered-disabled) for the selected agent.
    pub fn installed_for_selected_agent(&self) -> Vec<&InstalledMcp> {
        let Some(agent) = self.data.agents.get(self.agents_ui.agent_cursor) else {
            return Vec::new();
        };
        let mut rows: Vec<&InstalledMcp> = self
            .data
            .installed
            .iter()
            .filter(|i| i.agent == agent.id)
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.transport.cmp(&b.transport)));
        rows
    }

    /// Agents that can receive a global-scope install (they have a global path).
    pub fn installable_agents(&self) -> Vec<&AgentInfo> {
        self.data.agents.iter().filter(|a| a.has_global).collect()
    }

    pub fn installable_agents_for(&self, transport: &str) -> Vec<&AgentInfo> {
        self.installable_agents()
            .into_iter()
            .filter(|agent| agent.supported_transports.contains(&transport))
            .collect()
    }

    /// Catalog entries not already active in `agent_id`, matching `query`.
    pub fn addable_entries(&self, agent_id: &str, query: &str) -> Vec<&RegistryEntry> {
        let supported = self
            .data
            .agents
            .iter()
            .find(|agent| agent.id == agent_id)
            .map(|agent| &agent.supported_transports);
        let active: HashSet<(&str, &'static str)> = self
            .data
            .installed
            .iter()
            .filter(|i| i.enabled && i.agent == agent_id)
            .map(|i| (i.name.as_str(), transport_str(&i.transport)))
            .collect();
        let matcher = SkimMatcherV2::default();
        let q = query.trim();
        let mut out: Vec<&RegistryEntry> = self
            .data
            .registry
            .iter()
            .filter(|entry| {
                supported.is_some_and(|transports| transports.contains(&entry.transport()))
            })
            .filter(|e| !active.contains(&(e.name.as_str(), e.transport())))
            .filter(|e| {
                q.is_empty() || {
                    let hay = format!("{} {}", e.name, e.description);
                    matcher.fuzzy_match(&hay, q).is_some()
                }
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

/// Normalize a stored transport string to the interned `"stdio"`/`"http"`.
fn transport_str(t: &str) -> &'static str {
    if t == "stdio" {
        "stdio"
    } else {
        "http"
    }
}
