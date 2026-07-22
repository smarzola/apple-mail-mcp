use std::io::Write;

use clap::{Args, Parser, Subcommand};

use crate::{
    MailBackend, MailService,
    error::Result,
    model::{
        CheckMailRequest, DEFAULT_BODY_CHARS, DEFAULT_RESULT_LIMIT, GetMessageRequest,
        ListMailboxesRequest, MailboxRef, MessageRef, SearchRequest,
    },
};

#[derive(Debug, Parser)]
#[command(
    name = "apple-mail",
    version,
    about = "Control Apple Mail from the command line or MCP"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List configured Mail accounts without message content.
    Accounts,
    /// List mailboxes recursively.
    Mailboxes {
        /// Restrict results to a Mail account ID returned by `accounts`.
        #[arg(long)]
        account: Option<String>,
    },
    /// Search a mailbox and return bounded message metadata.
    Search(SearchArgs),
    /// Show one message, including a bounded plain-text body.
    Show(MessageArgs),
    /// Ask Mail to check all accounts or one account for new messages.
    Check {
        /// Mail account ID returned by `accounts`.
        #[arg(long)]
        account: Option<String>,
    },
}

#[derive(Clone, Debug, Args)]
pub struct MailboxArgs {
    /// Mail account ID. Omit it to use Mail's aggregate INBOX.
    #[arg(long)]
    pub account: Option<String>,
    /// One mailbox path component. Repeat for nested mailboxes.
    #[arg(long = "mailbox", value_name = "PATH_COMPONENT")]
    pub mailbox_path: Vec<String>,
}

impl MailboxArgs {
    pub fn reference(&self) -> MailboxRef {
        if self.mailbox_path.is_empty() {
            MailboxRef::inbox(self.account.clone())
        } else {
            MailboxRef {
                account_id: self.account.clone(),
                path: self.mailbox_path.clone(),
            }
        }
    }
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[command(flatten)]
    pub mailbox: MailboxArgs,
    /// Return only unread (`true`) or read (`false`) messages.
    #[arg(long)]
    pub unread: Option<bool>,
    /// Return only flagged (`true`) or unflagged (`false`) messages.
    #[arg(long)]
    pub flagged: Option<bool>,
    /// Case-insensitive substring match against the sender.
    #[arg(long)]
    pub sender: Option<String>,
    /// Case-insensitive substring match against the subject.
    #[arg(long)]
    pub subject: Option<String>,
    /// Maximum messages returned (1-100).
    #[arg(long, default_value_t = DEFAULT_RESULT_LIMIT)]
    pub limit: u16,
}

#[derive(Debug, Args)]
pub struct MessageArgs {
    #[command(flatten)]
    pub mailbox: MailboxArgs,
    /// Mail's positive local message ID returned by `search`.
    #[arg(long)]
    pub id: i64,
    /// Maximum body characters returned (1-65536).
    #[arg(long, default_value_t = DEFAULT_BODY_CHARS)]
    pub max_body_chars: u32,
}

impl MessageArgs {
    pub fn reference(&self) -> MessageRef {
        let mailbox = self.mailbox.reference();
        MessageRef {
            account_id: mailbox.account_id,
            mailbox_path: mailbox.path,
            id: self.id,
        }
    }
}

pub async fn run<B, W>(cli: Cli, service: &MailService<B>, output: &mut W) -> Result<()>
where
    B: MailBackend,
    W: Write,
{
    let value = match cli.command {
        Command::Accounts => serde_json::to_value(service.list_accounts().await?)?,
        Command::Mailboxes { account } => serde_json::to_value(
            service
                .list_mailboxes(ListMailboxesRequest {
                    account_id: account,
                })
                .await?,
        )?,
        Command::Search(args) => serde_json::to_value(
            service
                .search_messages(SearchRequest {
                    mailbox: args.mailbox.reference(),
                    unread: args.unread,
                    flagged: args.flagged,
                    sender_contains: args.sender,
                    subject_contains: args.subject,
                    limit: args.limit,
                })
                .await?,
        )?,
        Command::Show(args) => {
            let max_body_chars = args.max_body_chars;
            serde_json::to_value(
                service
                    .get_message(GetMessageRequest {
                        message: args.reference(),
                        max_body_chars,
                    })
                    .await?,
            )?
        }
        Command::Check { account } => serde_json::to_value(
            service
                .check_mail(CheckMailRequest {
                    account_id: account,
                })
                .await?,
        )?,
    };

    serde_json::to_writer_pretty(&mut *output, &value)?;
    writeln!(output)?;
    Ok(())
}
