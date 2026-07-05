//! The whole application state. Owned by the event loop; mutated only by
//! `update`; read only by `view`.

use std::collections::HashSet;
use std::time::Duration;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use mux_core::agents::AgentInfo;
use mux_core::ops::InstalledMcp;
use mux_core::sources::SourceView;
use mux_core::types::RegistryEntry;

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
        Self { query: String::new(), filter: OriginFilter::All, cursor: 0, searching: false }
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
        Self { agent_cursor: 0, installed_cursor: 0, pane: AgentPane::List }
    }
}

/// Overlay dialogs. Phase 2 has the read-only ones.
pub enum Modal {
    /// Read-only catalog-entry detail, keyed by `name::transport`.
    Detail { key: String },
    Help,
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
        let mut rows: Vec<&InstalledMcp> =
            self.data.installed.iter().filter(|i| i.agent == agent.id).collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.transport.cmp(&b.transport)));
        rows
    }
}
