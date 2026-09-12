//! Agent definition and configuration adapters; Core owns supported capabilities.

use std::path::PathBuf;

use clap::Subcommand;
use mux_core::application::agents::{self, AgentConfigurationPatch, AgentDefinition};
use mux_core::application::assets::PlanUpdateAgentCapabilitiesRequest;
use mux_core::application::operations::PlanOperationRequest;
use mux_core::application::MuxCore;
use serde_json::json;

use crate::command::{parse_identity, Cli};
use crate::input::json_file;
use crate::output::{CliError, CommandOutput};
use crate::review::{execute_direct_mutation, execute_operation};

#[derive(Debug, Subcommand)]
pub enum AgentManagementCommand {
    /// Save an AgentDefinition JSON document. Updating an existing ID requires --replace.
    Save {
        #[arg(value_parser = parse_identity)]
        id: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        replace: bool,
    },
    /// Apply an AgentConfigurationPatch JSON document through a reviewed capability plan.
    Configure {
        #[arg(value_parser = parse_identity)]
        agent: String,
        #[arg(long)]
        file: PathBuf,
    },
}

pub fn dispatch(cli: &Cli, command: &AgentManagementCommand) -> Result<CommandOutput, CliError> {
    let options = cli.mutation_options();
    options.validate()?;
    match command {
        AgentManagementCommand::Save { id, file, replace } => {
            let definition: AgentDefinition = serde_json::from_value(json_file(file)?)
                .map_err(|error| CliError::private("invalid_agent_definition", error.to_string()))?;
            execute_direct_mutation("agent.save", options,
                json!({"id": id, "name": definition.name, "replace": replace}),
                &format!("Save Agent {} ({id}); replace existing: {replace}. Core validates its global capability contract.", definition.name.as_deref().unwrap_or(id)),
                false, || agents::put(id.clone(), definition, *replace).map_err(CliError::from_legacy))
        }
        AgentManagementCommand::Configure { agent, file } => {
            let patch: AgentConfigurationPatch = serde_json::from_value(json_file(file)?)
                .map_err(|error| CliError::private("invalid_agent_configuration", error.to_string()))?;
            let plan = MuxCore::plan(PlanOperationRequest::UpdateAgentCapabilities(PlanUpdateAgentCapabilitiesRequest {
                agent_id: agent.clone(), patch,
            })).map_err(CliError::from_core)?;
            execute_operation("agent.configure", plan, options)
        }
    }
}
