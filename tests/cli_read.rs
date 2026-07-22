use std::sync::{Arc, Mutex};

use apple_mail_mcp::{
    MailBackend, MailService,
    cli::{Cli, run},
    model::{
        Account, CheckMailRequest, CheckMailResult, CompositionResult, CreateDraftRequest,
        GetMessageRequest, ListMailboxesRequest, Mailbox, MailboxRef, MessageDetail,
        MessageStateResult, MessageSummary, MoveMessageRequest, MoveMessageResult, SearchRequest,
        SendMessageRequest, SetMessageStateRequest,
    },
};
use async_trait::async_trait;
use clap::Parser;

#[derive(Clone, Default)]
struct FakeBackend {
    searches: Arc<Mutex<Vec<SearchRequest>>>,
    gets: Arc<Mutex<Vec<GetMessageRequest>>>,
    fail: bool,
}

#[async_trait]
impl MailBackend for FakeBackend {
    async fn list_accounts(&self) -> apple_mail_mcp::Result<Vec<Account>> {
        if self.fail {
            return Err(apple_mail_mcp::MailError::AutomationFailed(
                "classified failure".to_owned(),
            ));
        }
        Ok(vec![Account {
            id: "account-1".to_owned(),
            name: "Personal".to_owned(),
            email_addresses: vec!["person@example.com".to_owned()],
            enabled: true,
        }])
    }

    async fn list_mailboxes(
        &self,
        _: ListMailboxesRequest,
    ) -> apple_mail_mcp::Result<Vec<Mailbox>> {
        Ok(vec![Mailbox {
            reference: MailboxRef::inbox(Some("account-1".to_owned())),
            name: "INBOX".to_owned(),
            unread_count: 2,
        }])
    }

    async fn search_messages(
        &self,
        request: SearchRequest,
    ) -> apple_mail_mcp::Result<Vec<MessageSummary>> {
        self.searches.lock().unwrap().push(request);
        Ok(Vec::new())
    }

    async fn get_message(
        &self,
        request: GetMessageRequest,
    ) -> apple_mail_mcp::Result<MessageDetail> {
        self.gets.lock().unwrap().push(request.clone());
        Ok(MessageDetail {
            summary: MessageSummary {
                reference: request.message,
                message_id: Some("message@example.com".to_owned()),
                subject: "Hello".to_owned(),
                sender: "sender@example.com".to_owned(),
                date_received: Some("2026-07-22T10:00:00.000Z".to_owned()),
                read: false,
                flagged: true,
                size: 42,
            },
            content: "Hello world".to_owned(),
            content_truncated: false,
        })
    }

    async fn check_mail(&self, _: CheckMailRequest) -> apple_mail_mcp::Result<CheckMailResult> {
        Ok(CheckMailResult { requested: true })
    }

    async fn set_message_state(
        &self,
        _: SetMessageStateRequest,
    ) -> apple_mail_mcp::Result<MessageStateResult> {
        unreachable!("read-only test invoked state mutation")
    }

    async fn move_message(
        &self,
        _: MoveMessageRequest,
    ) -> apple_mail_mcp::Result<MoveMessageResult> {
        unreachable!("read-only test invoked move")
    }

    async fn create_draft(
        &self,
        _: CreateDraftRequest,
    ) -> apple_mail_mcp::Result<CompositionResult> {
        unreachable!("read-only test invoked draft creation")
    }

    async fn send_message(
        &self,
        _: SendMessageRequest,
    ) -> apple_mail_mcp::Result<CompositionResult> {
        unreachable!("read-only test invoked sending")
    }
}

#[tokio::test]
async fn accounts_emit_stable_json() {
    let service = MailService::new(FakeBackend::default());
    let cli = Cli::try_parse_from(["apple-mail", "accounts"]).unwrap();
    let mut output = Vec::new();

    run(cli, &service, &mut output).await.unwrap();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value[0]["id"], "account-1");
    assert_eq!(value[0]["email_addresses"][0], "person@example.com");
}

#[tokio::test]
async fn search_preserves_repeated_mailbox_components() {
    let backend = FakeBackend::default();
    let searches = backend.searches.clone();
    let service = MailService::new(backend);
    let cli = Cli::try_parse_from([
        "apple-mail",
        "search",
        "--account",
        "account-1",
        "--mailbox",
        "Projects",
        "--mailbox",
        "Customer",
        "--unread",
        "true",
        "--limit",
        "7",
    ])
    .unwrap();
    let mut output = Vec::new();

    run(cli, &service, &mut output).await.unwrap();

    let request = searches.lock().unwrap().pop().unwrap();
    assert_eq!(request.mailbox.account_id.as_deref(), Some("account-1"));
    assert_eq!(request.mailbox.path, ["Projects", "Customer"]);
    assert_eq!(request.unread, Some(true));
    assert_eq!(request.limit, 7);
}

#[tokio::test]
async fn mailboxes_show_and_check_emit_json() {
    let backend = FakeBackend::default();
    let gets = backend.gets.clone();
    let service = MailService::new(backend);

    let mut mailboxes = Vec::new();
    run(
        Cli::try_parse_from(["apple-mail", "mailboxes", "--account", "account-1"]).unwrap(),
        &service,
        &mut mailboxes,
    )
    .await
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&mailboxes).unwrap();
    assert_eq!(value[0]["reference"]["account_id"], "account-1");

    let mut shown = Vec::new();
    run(
        Cli::try_parse_from([
            "apple-mail",
            "show",
            "--account",
            "account-1",
            "--mailbox",
            "INBOX",
            "--id",
            "42",
            "--max-body-chars",
            "50",
        ])
        .unwrap(),
        &service,
        &mut shown,
    )
    .await
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&shown).unwrap();
    assert_eq!(value["content"], "Hello world");
    let request = gets.lock().unwrap().pop().unwrap();
    assert_eq!(request.message.id, 42);
    assert_eq!(request.max_body_chars, 50);

    let mut checked = Vec::new();
    run(
        Cli::try_parse_from(["apple-mail", "check"]).unwrap(),
        &service,
        &mut checked,
    )
    .await
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&checked).unwrap();
    assert_eq!(value["requested"], true);
}

#[tokio::test]
async fn validation_and_backend_errors_are_returned_without_partial_json() {
    let service = MailService::new(FakeBackend::default());
    let cli = Cli::try_parse_from(["apple-mail", "search", "--limit", "0"]).unwrap();
    let mut output = Vec::new();
    assert!(run(cli, &service, &mut output).await.is_err());
    assert!(output.is_empty());

    let service = MailService::new(FakeBackend {
        fail: true,
        ..FakeBackend::default()
    });
    let cli = Cli::try_parse_from(["apple-mail", "accounts"]).unwrap();
    assert!(run(cli, &service, &mut output).await.is_err());
    assert!(output.is_empty());
}
