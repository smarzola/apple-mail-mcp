use std::{
    sync::Arc,
    sync::atomic::{AtomicUsize, Ordering},
};

use apple_mail_mcp::{
    MailBackend, McpServer,
    error::Result,
    model::{
        Account, CheckMailRequest, CheckMailResult, CompositionResult, CreateDraftRequest,
        CreateReplyDraftRequest, GetMessageRequest, InboxSnapshot, InboxSnapshotRequest,
        ListMailboxesRequest, Mailbox, MessageDetail, MessageSearchResult, MessageStateResult,
        MoveMessageRequest, MoveMessageResult, ReplyDraftResult, SearchRequest, SendMessageRequest,
        SetMessageStateRequest,
    },
};
use async_trait::async_trait;
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::json;

#[derive(Default)]
struct FakeBackend {
    account_calls: AtomicUsize,
    write_calls: AtomicUsize,
    send_calls: AtomicUsize,
}

#[async_trait]
impl MailBackend for FakeBackend {
    async fn list_accounts(&self) -> Result<Vec<Account>> {
        if self.account_calls.fetch_add(1, Ordering::SeqCst) > 0 {
            return Err(apple_mail_mcp::MailError::AutomationFailed(
                "ACCOUNT-ID EMAIL@example.test MAILBOX SUBJECT SENDER MESSAGE-ID BODY".to_owned(),
            ));
        }
        Ok(vec![Account {
            id: "account-1".to_owned(),
            name: "Local Test".to_owned(),
            email_addresses: vec!["mail@example.test".to_owned()],
            enabled: true,
        }])
    }

    async fn list_mailboxes(&self, _: ListMailboxesRequest) -> Result<Vec<Mailbox>> {
        unreachable!("not called by this test")
    }

    async fn search_messages(&self, _: SearchRequest) -> Result<MessageSearchResult> {
        unreachable!("not called by this test")
    }

    async fn inbox_snapshot(&self, _: InboxSnapshotRequest) -> Result<InboxSnapshot> {
        unreachable!("not called by this test")
    }

    async fn get_message(&self, _: GetMessageRequest) -> Result<MessageDetail> {
        unreachable!("not called by this test")
    }

    async fn check_mail(&self, _: CheckMailRequest) -> Result<CheckMailResult> {
        self.write_calls.fetch_add(1, Ordering::SeqCst);
        Ok(CheckMailResult { requested: true })
    }

    async fn set_message_state(&self, _: SetMessageStateRequest) -> Result<MessageStateResult> {
        self.write_calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("default policy must reject before calling the backend")
    }

    async fn move_message(&self, _: MoveMessageRequest) -> Result<MoveMessageResult> {
        self.write_calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("default policy must reject before calling the backend")
    }

    async fn create_draft(&self, _: CreateDraftRequest) -> Result<CompositionResult> {
        self.write_calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("default policy must reject before calling the backend")
    }

    async fn create_reply_draft(&self, _: CreateReplyDraftRequest) -> Result<ReplyDraftResult> {
        self.write_calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("default policy must reject before calling the backend")
    }

    async fn send_message(&self, _: SendMessageRequest) -> Result<CompositionResult> {
        self.send_calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("default policy must reject before calling the backend")
    }
}

#[tokio::test]
async fn initializes_lists_calls_and_closes_over_stdio_framing() {
    let backend = Arc::new(FakeBackend::default());
    let server = McpServer::new(backend.clone(), false, false);
    let (server_stdio, client_stdio) = tokio::io::duplex(64 * 1024);

    let server_task = tokio::spawn(async move {
        server
            .serve(server_stdio)
            .await
            .expect("server should initialize")
            .waiting()
            .await
            .expect("server should close cleanly");
    });

    let client = ().serve(client_stdio).await.expect("client should initialize");
    let tools = client
        .peer()
        .list_tools(None)
        .await
        .expect("tools/list should succeed");

    assert_eq!(tools.tools.len(), 12);
    assert!(tools.tools.iter().all(|tool| tool.output_schema.is_some()));
    let send_tool = tools
        .tools
        .iter()
        .find(|tool| tool.name == "send_message")
        .expect("send tool should be described");
    let send_annotations = send_tool.annotations.as_ref().expect("annotations");
    assert_eq!(send_annotations.read_only_hint, Some(false));
    assert_eq!(send_annotations.destructive_hint, Some(true));

    let accounts = client
        .peer()
        .call_tool(CallToolRequestParams::new("list_accounts"))
        .await
        .expect("list_accounts should be a protocol success");
    assert_eq!(accounts.is_error, Some(false));
    assert_eq!(
        accounts.structured_content,
        Some(json!({
            "accounts": [{
                "id": "account-1",
                "name": "Local Test",
                "email_addresses": ["mail@example.test"],
                "enabled": true
            }]
        }))
    );
    assert!(
        !accounts.content.is_empty(),
        "text fallback should be present"
    );

    let doctor = client
        .peer()
        .call_tool(CallToolRequestParams::new("doctor"))
        .await
        .expect("doctor should succeed");
    assert_eq!(doctor.is_error, Some(false));
    let doctor_text = doctor
        .content
        .first()
        .and_then(|content| content.as_text())
        .expect("doctor text")
        .text
        .as_str();
    assert!(!doctor_text.contains("account-1"));
    assert!(!doctor_text.contains("mail@example.test"));
    assert!(doctor_text.contains("\"backend_ready\":false"));
    assert!(doctor_text.contains("\"diagnostic\":\"automation_failed\""));
    for secret in [
        "ACCOUNT-ID",
        "EMAIL@example.test",
        "MAILBOX",
        "SUBJECT",
        "SENDER",
        "MESSAGE-ID",
        "BODY",
    ] {
        assert!(!doctor_text.contains(secret));
    }

    let denied_writes = [
        ("check_mail", json!({})),
        (
            "set_message_state",
            json!({
                "message": {
                    "account_id": "account-1",
                    "mailbox_path": ["INBOX"],
                    "id": 1
                },
                "read": true,
                "flagged": null
            }),
        ),
        (
            "move_message",
            json!({
                "message": {
                    "account_id": "account-1",
                    "mailbox_path": ["INBOX"],
                    "id": 1
                },
                "destination": {
                    "account_id": "account-1",
                    "path": ["Archive"]
                },
                "confirm": true
            }),
        ),
        (
            "create_draft",
            json!({
                "message": {
                    "to": ["recipient@example.test"],
                    "cc": [],
                    "bcc": [],
                    "subject": "Draft",
                    "body": "Body"
                }
            }),
        ),
        (
            "create_reply_draft",
            json!({
                "message": {
                    "account_id": "account-1",
                    "mailbox_path": ["INBOX"],
                    "id": 1
                },
                "body": "Reply"
            }),
        ),
    ];
    for (name, arguments) in denied_writes {
        let denied = client
            .peer()
            .call_tool(
                CallToolRequestParams::new(name)
                    .with_arguments(arguments.as_object().expect("object").clone()),
            )
            .await
            .expect("policy denial should be a tool result");
        assert_eq!(denied.is_error, Some(true), "{name} should be denied");
    }
    assert_eq!(backend.write_calls.load(Ordering::SeqCst), 0);

    let send_arguments = json!({
        "message": {
            "to": ["recipient@example.test"],
            "cc": [],
            "bcc": [],
            "subject": "Policy test",
            "body": "This must not be sent"
        },
        "confirm": true
    })
    .as_object()
    .expect("object")
    .clone();
    let denied = client
        .peer()
        .call_tool(CallToolRequestParams::new("send_message").with_arguments(send_arguments))
        .await
        .expect("policy denial should be a tool result");
    assert_eq!(denied.is_error, Some(true));
    assert!(
        denied
            .content
            .first()
            .and_then(|content| content.as_text())
            .is_some_and(|text| text.text.contains("writes are disabled"))
    );
    assert_eq!(backend.send_calls.load(Ordering::SeqCst), 0);

    client.cancel().await.expect("client should close");
    server_task.await.expect("server task should join");
}
