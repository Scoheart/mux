#[derive(Debug, Clone, PartialEq)]
pub enum DiffAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffEntry {
    pub action: DiffAction,
    pub mcp_name: String,
    pub agent: String,
    pub scope: String,
}
