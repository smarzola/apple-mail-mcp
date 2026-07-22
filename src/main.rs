use std::{process::ExitCode, sync::Arc};

use apple_mail_mcp::{JxaBackend, MailService, cli, mcp};
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    let args = cli::Cli::parse();
    let result = match args.command {
        cli::Command::Mcp { allow_send } => {
            mcp::serve_stdio(Arc::new(JxaBackend::default()), allow_send)
                .await
                .map_err(|error| error.to_string())
        }
        command => {
            let service = MailService::new(JxaBackend::default()).with_send_enabled(true);
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            cli::run(cli::Cli { command }, &service, &mut output)
                .await
                .map_err(|error| error.to_string())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
