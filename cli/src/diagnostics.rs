//! Read-only diagnostic adapters. Core owns discovery, file checks, pagination
//! and credential redaction; CLI never independently parses Agent logs.
use std::path::PathBuf;
use clap::Subcommand;
use mux_core::application::{capture, traces};
use serde_json::{json, Value};
use crate::command::Cli;
use crate::output::{query_output, CliError, CommandOutput};
use crate::projection::{safe_path, safe_url};

#[derive(Debug, Subcommand)]
pub enum TraceCommand {
    /// List locally discovered conversation sessions without reading full bodies.
    List {
        #[arg(long)] agent: Option<String>,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: u16,
    },
    /// Read a page or an exact revision-bound event. Imported files are local only.
    Show {
        session: Option<String>,
        #[arg(long, conflicts_with = "session", required_unless_present = "session")]
        file: Option<PathBuf>,
        #[arg(long, conflicts_with = "event")] cursor: Option<String>,
        #[arg(long, requires = "revision")] event: Option<String>,
        #[arg(long, requires = "event")] revision: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum CaptureCommand {
    /// List saved capture sessions. Does not start or intercept any traffic.
    List {
        #[arg(long)] agent: Option<String>,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: u16,
    },
    /// List saved request summaries, or inspect one redacted request/response.
    Show {
        session: String,
        #[arg(long)] flow: Option<String>,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=5000))]
        limit: u16,
    },
}

fn trace_session(session: &traces::TraceSession) -> Value {
    json!({"id": session.id, "source_id": session.source_id, "agent_name": session.agent_name,
        "format": session.format, "title": session.title,
        "project": session.project.as_deref().map(safe_path), "path": safe_path(&session.path),
        "modified_at": session.modified_at, "bytes": session.bytes, "imported": session.imported})
}

pub fn trace(cli: &Cli, command: &TraceCommand) -> Result<CommandOutput, CliError> {
    cli.reject_mutation_options()?;
    match command {
        TraceCommand::List { agent, limit } => {
            let index = traces::index().map_err(CliError::from_legacy)?;
            let sessions = index.sessions.iter().filter(|session| agent.as_ref().is_none_or(|agent|
                index.sources.iter().any(|source| source.id == session.source_id && source.agent_ids.contains(agent))
            )).collect::<Vec<_>>();
            query_output("trace.list", json!({"total": sessions.len(),
                "sessions": sessions.iter().take(usize::from(*limit)).map(|session| trace_session(session)).collect::<Vec<_>>(),
                "sources": index.sources, "warnings": index.warnings}))
        }
        TraceCommand::Show { session, file, cursor, event, revision } => {
            // Locator IDs are process-local. Re-establish the trusted locator
            // each invocation instead of accepting an ID as a filesystem path.
            let id = if let Some(file) = file {
                traces::import(file).map_err(CliError::from_legacy)?.id
            } else {
                let id = session.as_ref().ok_or_else(|| CliError::new("session_required", "choose a session or --file"))?;
                let index = traces::index().map_err(CliError::from_legacy)?;
                if !index.sessions.iter().any(|session| &session.id == id) {
                    return Err(CliError::new("session_not_found", "session is no longer in the local trace index"));
                }
                id.clone()
            };
            if let Some(event) = event {
                let revision = revision.as_deref().ok_or_else(|| CliError::new("revision_required", "event detail requires the page revision"))?;
                let detail = traces::detail(&id, event, revision).map_err(CliError::from_legacy)?;
                query_output("trace.show", serde_json::to_value(detail)
                    .map_err(|error| CliError::private("serialization", error.to_string()))?)
            } else {
                let page = traces::page(&id, cursor.as_deref()).map_err(CliError::from_legacy)?;
                query_output("trace.show", json!({"session": trace_session(&page.session),
                    "events": page.events, "next_cursor": page.next_cursor,
                    "revision": page.revision, "warnings": page.warnings}))
            }
        }
    }
}

pub fn captured(cli: &Cli, command: &CaptureCommand) -> Result<CommandOutput, CliError> {
    cli.reject_mutation_options()?;
    match command {
        CaptureCommand::List { agent, limit } => {
            let sessions = capture::sessions().map_err(CliError::from_legacy)?;
            let sessions = sessions.iter().filter(|session| agent.as_ref().is_none_or(|agent| &session.agent_id == agent)).collect::<Vec<_>>();
            query_output("capture.list", json!({"total": sessions.len(), "sessions": sessions.iter()
                .take(usize::from(*limit)).map(|session| json!({"id": session.id,
                    "agent_id": session.agent_id, "agent_name": session.agent_name,
                    "target_name": session.target_name, "started_at": session.started_at,
                    "proxy_url": session.proxy_url.as_deref().map(safe_url)})).collect::<Vec<_>>()}))
        }
        CaptureCommand::Show { session, flow, limit } => {
            if let Some(flow) = flow {
                let record = capture::detail(session, flow).map_err(CliError::from_legacy)?;
                query_output("capture.show", serde_json::to_value(record)
                    .map_err(|error| CliError::private("serialization", error.to_string()))?)
            } else {
                // Do not infer another process's live state from this CLI's
                // empty ACTIVE worker. These are explicitly persisted records.
                let summaries = capture::summaries(session).map_err(CliError::from_legacy)?;
                query_output("capture.show", json!({"session_id": session, "source": "saved_records",
                    "total": summaries.len(), "flows": summaries.into_iter().take(usize::from(*limit)).collect::<Vec<_>>()}))
            }
        }
    }
}
