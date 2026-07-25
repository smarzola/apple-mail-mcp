use std::sync::{Arc, Mutex};

use apple_mail_mcp::{
    MailBackend, MailService,
    cli::{Cli, run},
    model::{
        Account, CheckMailRequest, CheckMailResult, CompositionResult, CreateDraftRequest,
        CreateReplyDraftRequest, GetMessageRequest, InboxSnapshot, InboxSnapshotRequest,
        ListMailboxesRequest, Mailbox, MessageDetail, MessageSearchResult, MessageStateResult,
        MoveMessageRequest, MoveMessageResult, ReplyDraftResult, SearchRequest, SendMessageRequest,
        SetMessageStateRequest,
    },
};
use async_trait::async_trait;
use clap::Parser;

#[derive(Clone, Default)]
struct FakeBackend {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl MailBackend for FakeBackend {
    async fn list_accounts(&self) -> apple_mail_mcp::Result<Vec<Account>> {
        unreachable!("write test invoked accounts")
    }

    async fn list_mailboxes(
        &self,
        _: ListMailboxesRequest,
    ) -> apple_mail_mcp::Result<Vec<Mailbox>> {
        unreachable!("write test invoked mailboxes")
    }

    async fn search_messages(
        &self,
        _: SearchRequest,
    ) -> apple_mail_mcp::Result<MessageSearchResult> {
        unreachable!("write test invoked search")
    }

    async fn inbox_snapshot(
        &self,
        _: InboxSnapshotRequest,
    ) -> apple_mail_mcp::Result<InboxSnapshot> {
        unreachable!("write test invoked inbox snapshot")
    }

    async fn get_message(&self, _: GetMessageRequest) -> apple_mail_mcp::Result<MessageDetail> {
        unreachable!("write test invoked show")
    }

    async fn check_mail(&self, _: CheckMailRequest) -> apple_mail_mcp::Result<CheckMailResult> {
        unreachable!("write test invoked check")
    }

    async fn set_message_state(
        &self,
        request: SetMessageStateRequest,
    ) -> apple_mail_mcp::Result<MessageStateResult> {
        self.calls.lock().unwrap().push(format!(
            "state:{}:{:?}:{:?}",
            request.message.id, request.read, request.flagged
        ));
        Ok(MessageStateResult {
            message: request.message,
            read: request.read.unwrap_or(false),
            flagged: request.flagged.unwrap_or(false),
        })
    }

    async fn move_message(
        &self,
        request: MoveMessageRequest,
    ) -> apple_mail_mcp::Result<MoveMessageResult> {
        self.calls.lock().unwrap().push(format!(
            "move:{}:{}",
            request.destination.account_id.as_deref().unwrap_or("none"),
            request.destination.path.join("/")
        ));
        Ok(MoveMessageResult {
            moved: true,
            destination: request.destination,
        })
    }

    async fn create_draft(
        &self,
        request: CreateDraftRequest,
    ) -> apple_mail_mcp::Result<CompositionResult> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("draft:{}", request.message.to.join(",")));
        Ok(CompositionResult {
            local_id: 10,
            sent: false,
            visible: true,
        })
    }

    async fn create_reply_draft(
        &self,
        request: CreateReplyDraftRequest,
    ) -> apple_mail_mcp::Result<ReplyDraftResult> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("reply:{}:{}", request.message.id, request.body));
        Ok(ReplyDraftResult {
            source: request.message,
            local_id: 12,
            subject: "Re: Hello".to_owned(),
            to: vec!["sender@example.com".to_owned()],
            cc: Vec::new(),
            sent: false,
            draft_present: true,
            visible: true,
            content_verified: true,
        })
    }

    async fn send_message(
        &self,
        request: SendMessageRequest,
    ) -> apple_mail_mcp::Result<CompositionResult> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("send:{}", request.message.to.join(",")));
        Ok(CompositionResult {
            local_id: 11,
            sent: true,
            visible: false,
        })
    }
}

#[tokio::test]
async fn state_move_and_draft_use_typed_requests() {
    let backend = FakeBackend::default();
    let calls = backend.calls.clone();
    let service = MailService::new(backend).with_write_enabled(true);

    let commands = [
        vec![
            "apple-mail",
            "state",
            "--account",
            "account-1",
            "--mailbox",
            "INBOX",
            "--id",
            "7",
            "--read",
            "true",
            "--flagged",
            "false",
        ],
        vec![
            "apple-mail",
            "move",
            "--account",
            "account-1",
            "--mailbox",
            "INBOX",
            "--id",
            "7",
            "--destination-mailbox",
            "Archive",
            "--confirm-move",
        ],
        vec![
            "apple-mail",
            "draft",
            "--to",
            "person@example.com",
            "--subject",
            "Hello",
            "--body",
            "Body",
        ],
        vec![
            "apple-mail",
            "reply",
            "--account",
            "account-1",
            "--mailbox",
            "INBOX",
            "--id",
            "7",
            "--body",
            "Reply body",
        ],
    ];

    for command in commands {
        let mut output = Vec::new();
        run(Cli::try_parse_from(command).unwrap(), &service, &mut output)
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_slice(&output).unwrap();
    }

    assert_eq!(
        *calls.lock().unwrap(),
        [
            "state:7:Some(true):Some(false)",
            "move:account-1:Archive",
            "draft:person@example.com",
            "reply:7:Reply body"
        ]
    );
}

#[tokio::test]
async fn send_requires_policy_and_cli_confirmation() {
    let backend = FakeBackend::default();
    let calls = backend.calls.clone();
    let args = [
        "apple-mail",
        "send",
        "--to",
        "person@example.com",
        "--subject",
        "Hello",
    ];

    let denied = MailService::new(backend.clone());
    let mut output = Vec::new();
    assert!(
        run(Cli::try_parse_from(args).unwrap(), &denied, &mut output)
            .await
            .is_err()
    );

    let allowed = MailService::new(backend)
        .with_write_enabled(true)
        .with_send_enabled(true);
    assert!(
        run(Cli::try_parse_from(args).unwrap(), &allowed, &mut output)
            .await
            .is_err()
    );
    let confirmed = [
        "apple-mail",
        "send",
        "--to",
        "person@example.com",
        "--subject",
        "Hello",
        "--confirm-send",
    ];
    run(
        Cli::try_parse_from(confirmed).unwrap(),
        &allowed,
        &mut output,
    )
    .await
    .unwrap();

    assert_eq!(*calls.lock().unwrap(), ["send:person@example.com"]);
}

#[tokio::test]
async fn invalid_recipient_fails_without_backend_call() {
    let backend = FakeBackend::default();
    let calls = backend.calls.clone();
    let service = MailService::new(backend).with_write_enabled(true);
    let args = ["apple-mail", "draft", "--to", "not-an-address"];
    let mut output = Vec::new();

    assert!(
        run(Cli::try_parse_from(args).unwrap(), &service, &mut output)
            .await
            .is_err()
    );
    assert!(calls.lock().unwrap().is_empty());
    assert!(output.is_empty());
}

#[tokio::test]
async fn unscoped_move_is_rejected_without_backend_call() {
    let backend = FakeBackend::default();
    let calls = backend.calls.clone();
    let service = MailService::new(backend).with_write_enabled(true);
    let args = [
        "apple-mail",
        "move",
        "--mailbox",
        "INBOX",
        "--id",
        "7",
        "--destination-mailbox",
        "Archive",
        "--confirm-move",
    ];
    let mut output = Vec::new();

    assert!(
        run(Cli::try_parse_from(args).unwrap(), &service, &mut output)
            .await
            .is_err()
    );
    assert!(calls.lock().unwrap().is_empty());
    assert!(output.is_empty());
}

#[tokio::test]
async fn move_requires_cli_confirmation_without_backend_call() {
    let backend = FakeBackend::default();
    let calls = backend.calls.clone();
    let service = MailService::new(backend).with_write_enabled(true);
    let args = [
        "apple-mail",
        "move",
        "--account",
        "account-1",
        "--mailbox",
        "INBOX",
        "--id",
        "7",
        "--destination-mailbox",
        "Archive",
    ];
    let mut output = Vec::new();

    assert!(
        run(Cli::try_parse_from(args).unwrap(), &service, &mut output)
            .await
            .is_err()
    );
    assert!(calls.lock().unwrap().is_empty());
    assert!(output.is_empty());
}

#[test]
fn mcp_send_opt_in_requires_write_opt_in() {
    assert!(Cli::try_parse_from(["apple-mail", "mcp", "--allow-send"]).is_err());
    assert!(Cli::try_parse_from(["apple-mail", "mcp", "--allow-write", "--allow-send"]).is_ok());
}
