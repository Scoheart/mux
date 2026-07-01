#[derive(Debug, Clone, PartialEq)]
pub enum DiffAction { Add, Remove }

#[derive(Debug, Clone, PartialEq)]
pub struct DiffEntry {
    pub action: DiffAction,
    pub mcp_name: String,
    pub agent: String,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct DesiredMcp {
    pub name: String,
    pub agents: Vec<String>,
    pub scopes: Vec<String>, // expanded global/project
}

#[derive(Debug, Clone)]
pub struct CurrentMcp {
    pub name: String,
    pub agent: String,
    pub scope: String,
}

pub fn compute_diff(desired: &[DesiredMcp], current: &[CurrentMcp]) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();
    let mut desired_set = std::collections::HashSet::new();
    for d in desired {
        for scope in &d.scopes {
            for agent in &d.agents {
                let key = format!("{}|{}|{}", d.name, agent, scope);
                desired_set.insert(key.clone());
                let exists = current.iter().any(|c|
                    c.name == d.name && &c.agent == agent && &c.scope == scope);
                if !exists {
                    diffs.push(DiffEntry { action: DiffAction::Add,
                        mcp_name: d.name.clone(), agent: agent.clone(), scope: scope.clone() });
                }
            }
        }
    }
    for c in current {
        let key = format!("{}|{}|{}", c.name, c.agent, c.scope);
        if !desired_set.contains(&key) {
            diffs.push(DiffEntry { action: DiffAction::Remove,
                mcp_name: c.name.clone(), agent: c.agent.clone(), scope: c.scope.clone() });
        }
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adds_missing_and_removes_extra() {
        let desired = vec![DesiredMcp {
            name: "git".into(), agents: vec!["claude-code".into()],
            scopes: vec!["global".into()] }];
        let current = vec![CurrentMcp {
            name: "old".into(), agent: "claude-code".into(), scope: "global".into() }];
        let diffs = compute_diff(&desired, &current);
        assert!(diffs.iter().any(|d| d.action == DiffAction::Add && d.mcp_name == "git"));
        assert!(diffs.iter().any(|d| d.action == DiffAction::Remove && d.mcp_name == "old"));
    }
}
