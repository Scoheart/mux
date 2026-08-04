//! MUX CLI — a thin clap/TUI front-end over `mux-core`.

use std::ffi::OsString;

use clap::{CommandFactory, Parser};
use serde_json::json;

mod command;
mod output;
mod projection;
mod review;
mod tui;

use command::{dispatch, Cli};
use output::{render_error, render_success, CliError};

fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    let json_mode = requested_json_mode(&args);
    let no_color = json_mode || args.iter().any(|argument| argument == "--no-color");
    let cli = match parse(args, json_mode) {
        Ok(ParseOutcome::Run(cli)) => cli,
        Ok(ParseOutcome::Early(output)) => {
            if let Err(error) = render_success(&output, true) {
                render_error(&error, true, true);
                std::process::exit(1);
            }
            return;
        }
        Err(error) => {
            render_error(&error, json_mode, no_color);
            std::process::exit(2);
        }
    };
    let (json_mode, no_color) = runtime_output_modes(&cli);
    if let Err(error) = run(cli) {
        render_error(&error, json_mode, no_color);
        std::process::exit(1);
    }
}

fn runtime_output_modes(cli: &Cli) -> (bool, bool) {
    (cli.json, cli.json || cli.no_color)
}

#[derive(Debug)]
enum ParseOutcome {
    Run(Cli),
    Early(output::CommandOutput),
}

fn requested_json_mode(args: &[OsString]) -> bool {
    let mut argument_value = false;
    for argument in args.iter().skip(1) {
        if argument_value {
            argument_value = false;
            continue;
        }
        if argument == "--" {
            break;
        }
        if argument == "--arg" {
            argument_value = true;
            continue;
        }
        if argument == "--json" {
            return true;
        }
    }
    false
}

fn parse(args: Vec<OsString>, json_mode: bool) -> Result<ParseOutcome, CliError> {
    match Cli::try_parse_from(args) {
        Ok(cli) => Ok(ParseOutcome::Run(cli)),
        Err(error) if error.use_stderr() => {
            if json_mode {
                Err(CliError::private("invalid_arguments", error.to_string()))
            } else {
                error.exit()
            }
        }
        Err(error) if json_mode => {
            let kind = error.kind();
            let (command, data) = if kind == clap::error::ErrorKind::DisplayVersion {
                (
                    "version",
                    json!({"name": "mux", "version": env!("CARGO_PKG_VERSION")}),
                )
            } else {
                ("help", json!({"text": error.to_string()}))
            };
            Ok(ParseOutcome::Early(output::CommandOutput::new(
                command, false, data, "",
            )))
        }
        Err(error) => {
            let _ = error.print();
            std::process::exit(0);
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    if cli.command.is_none() {
        if cli.json || cli.yes || cli.dry_run || cli.no_color {
            return Err(CliError::new(
                "command_required",
                "global flags require an explicit command",
            ));
        }
        if std::env::var_os("MUX_NO_TUI").is_some() {
            Cli::command()
                .print_help()
                .map_err(|error| CliError::private("help_output_failed", error.to_string()))?;
            println!();
            return Ok(());
        }
        bootstrap(false)?;
        return tui::run().map_err(|error| CliError::private("tui_failed", error.to_string()));
    }

    let report = bootstrap(cli.json)?;
    if command_requires_model_mutation(cli.command.as_ref()) {
        reject_model_blocker(&report)?;
    }

    let is_upgrade = matches!(cli.command, Some(command::Command::Upgrade));
    let output = dispatch(&cli)?;
    render_success(&output, cli.json)?;

    if !cli.json && !is_upgrade {
        if let Some(notice) =
            mux_core::application::update::passive_check_notice(env!("CARGO_PKG_VERSION"))
        {
            eprintln!("\n{notice}");
        }
    }
    Ok(())
}

fn bootstrap(
    json_mode: bool,
) -> Result<mux_core::application::bootstrap::BootstrapReport, CliError> {
    let outcome =
        mux_core::application::MuxCore::bootstrap(mux_core::application::bootstrap::Frontend::Cli)
            .map_err(|error| {
                CliError::new("bootstrap_failed", "MUX startup failed")
                    .with_detail("stage", error.stage.code())
                    .with_detail("reviewable", false)
            })?;
    if !json_mode {
        for warning in &outcome.warnings {
            eprintln!("MUX startup warning: {}", warning.message);
        }
    }
    Ok(outcome)
}

fn command_requires_model_mutation(command: Option<&command::Command>) -> bool {
    match command {
        Some(command::Command::Model { command }) => !matches!(
            command,
            command::ModelCommand::List
                | command::ModelCommand::Show { .. }
                | command::ModelCommand::Status { .. }
        ),
        Some(command::Command::Adopt {
            command: command::AdoptCommand::Model { .. },
        }) => true,
        _ => false,
    }
}

fn reject_model_blocker(
    report: &mux_core::application::bootstrap::BootstrapReport,
) -> Result<(), CliError> {
    match &report.status {
        mux_core::application::BackendStatus::MigrationReviewRequired { stage, .. } => {
            let review = mux_core::application::MuxCore::migration_review().ok_or_else(|| {
                CliError::new(
                    "migration_review_unavailable",
                    "migration review is unavailable",
                )
            })?;
            Err(CliError::new(
                "migration_review_required",
                "Model configuration upgrade needs confirmation; run `mux migration review`",
            )
            .with_detail("stage", stage.clone())
            .with_detail("capability", "model")
            .with_detail("reviewable", true)
            .with_detail("review", serde_json::to_value(review).unwrap_or_default()))
        }
        mux_core::application::BackendStatus::CapabilityUnavailable {
            capability: mux_core::application::CapabilityDomain::Model,
            stage,
            code,
            message,
        } => Err(CliError::new(code.clone(), message.clone())
            .with_detail("stage", stage.clone())
            .with_detail("capability", "model")
            .with_detail("reviewable", false)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_parse_error_becomes_structured_cli_error() {
        let error = parse(vec!["mux".into(), "--json".into(), "list".into()], true).unwrap_err();
        assert_eq!(error.code, "invalid_arguments");
    }

    #[test]
    fn json_looking_server_arg_does_not_select_json_output() {
        let ParseOutcome::Run(cli) = parse(
            vec![
                "mux".into(),
                "mcp".into(),
                "add".into(),
                "edge::stdio".into(),
                "--command".into(),
                "npx".into(),
                "--arg".into(),
                "--json".into(),
            ],
            true,
        )
        .unwrap() else {
            panic!("expected parsed CLI");
        };
        assert_eq!(runtime_output_modes(&cli), (false, false));
    }

    #[test]
    fn option_aware_json_detection_skips_server_argument_values() {
        let args = vec![
            "mux".into(),
            "mcp".into(),
            "add".into(),
            "invalid".into(),
            "--arg".into(),
            "--json".into(),
        ];
        assert!(!requested_json_mode(&args));
        assert!(requested_json_mode(&[
            "mux".into(),
            "--json".into(),
            "mcp".into(),
            "list".into(),
        ]));
    }

    #[test]
    fn model_blocker_precheck_is_limited_to_model_mutations() {
        let model_list = Cli::try_parse_from(["mux", "model", "list"]).unwrap();
        assert!(!command_requires_model_mutation(
            model_list.command.as_ref()
        ));

        let model_assign =
            Cli::try_parse_from(["mux", "model", "assign", "profile", "--agent", "codex"]).unwrap();
        assert!(command_requires_model_mutation(
            model_assign.command.as_ref()
        ));

        let skill_assign =
            Cli::try_parse_from(["mux", "skill", "assign", "skill", "--agent", "codex"]).unwrap();
        assert!(!command_requires_model_mutation(
            skill_assign.command.as_ref()
        ));

        let model_adopt = Cli::try_parse_from(["mux", "adopt", "model", "candidate"]).unwrap();
        assert!(command_requires_model_mutation(
            model_adopt.command.as_ref()
        ));
    }
}
