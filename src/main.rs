use std::process::ExitCode;

use apple_mail_mcp::{JxaBackend, MailService, cli};
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    let args = cli::Cli::parse();
    let service = MailService::new(JxaBackend::default()).with_send_enabled(true);
    let stdout = std::io::stdout();
    let mut output = stdout.lock();

    match cli::run(args, &service, &mut output).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
