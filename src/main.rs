mod server;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "mdreview", version, about)]
struct Cli {
    /// Project folder containing Markdown files.
    #[arg(default_value = ".", global = true)]
    project: PathBuf,

    /// Print the URL without opening the default browser.
    #[arg(long, global = true)]
    no_open: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize review metadata and optional AGENTS.md instructions.
    Init {
        /// Append a managed review block when AGENTS.md already exists.
        #[arg(long)]
        append: bool,
    },
    /// List review comments as JSON.
    Comments {
        #[arg(long)]
        open: bool,
        #[arg(long)]
        document: Option<String>,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Start a Markdown revision task and print its complete agent instructions.
    Revise { id: String },
    /// Inspect or submit an agent review task.
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ReviewCommand {
    /// Print a review task and its comments.
    Task {
        id: String,
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Submit candidate revision results for human review.
    Submit {
        id: String,
        /// JSON dispositions for mixed results or comments needing clarification.
        #[arg(
            long,
            conflicts_with = "addressed_all",
            required_unless_present = "addressed_all"
        )]
        report: Option<PathBuf>,
        /// Mark every comment in the task addressed without creating a report file.
        #[arg(long, conflicts_with = "report")]
        addressed_all: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        None => server::run(cli.project, cli.no_open)
            .await
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Some(Command::Init { append }) => server::commands::initialize(cli.project, append)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Some(Command::Comments {
            open,
            document,
            format,
        }) => {
            if format != "json" {
                Err("only --format json is currently supported".into())
            } else {
                server::commands::list_comments(cli.project, open, document)
                    .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            }
        }
        Some(Command::Revise { id }) => server::commands::revise_task(cli.project, &id)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        Some(Command::Review { command }) => match command {
            ReviewCommand::Task { id, format } => {
                if format != "json" {
                    Err("only --format json is currently supported".into())
                } else {
                    server::commands::show_task(cli.project, &id)
                        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
                }
            }
            ReviewCommand::Submit {
                id,
                report,
                addressed_all,
            } => server::commands::submit_task(cli.project, &id, report, addressed_all)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        },
    };

    if let Err(error) = result {
        eprintln!("mdreview: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_submit_requires_exactly_one_result_mode() {
        assert!(
            Cli::try_parse_from([
                "mdreview",
                "review",
                "submit",
                "task_123",
                "--addressed-all",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "mdreview",
                "review",
                "submit",
                "task_123",
                "--report",
                "report.json",
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["mdreview", "review", "submit", "task_123"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdreview",
                "review",
                "submit",
                "task_123",
                "--addressed-all",
                "--report",
                "report.json",
            ])
            .is_err()
        );
    }
}
