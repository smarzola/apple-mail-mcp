use std::{
    sync::Arc,
    sync::atomic::{AtomicUsize, Ordering},
};

use apple_mail_mcp::{
    MailBackend, McpServer,
    error::Result,
    model::{
        Account, CheckMailRequest, CheckMailResult, CompositionResult, CreateDraftRequest,
        GetMessageRequest, InboxSnapshot, InboxSnapshotRequest, ListMailboxesRequest, Mailbox,
        MessageDetail, MessageSearchResult, MessageStateResult, MoveMessageRequest,
        MoveMessageResult, SearchRequest, SendMessageRequest, SetMessageStateRequest,
    },
};
use async_trait::async_trait;
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::json;

#[derive(Default)]
struct FakeBackend {
    send_calls: AtomicUsize,
}

#[async_trait]
impl MailBackend for FakeBackend {
    async fn list_accounts(&self) -> Result<Vec<Account>> {
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
        unreachable!("not called by this test")
    }

    async fn set_message_state(&self, _: SetMessageStateRequest) -> Result<MessageStateResult> {
        unreachable!("not called by this test")
    }

    async fn move_message(&self, _: MoveMessageRequest) -> Result<MoveMessageResult> {
        unreachable!("not called by this test")
    }

    async fn create_draft(&self, _: CreateDraftRequest) -> Result<CompositionResult> {
        unreachable!("not called by this test")
    }

    async fn send_message(&self, _: SendMessageRequest) -> Result<CompositionResult> {
        self.send_calls.fetch_add(1, Ordering::SeqCst);
        unreachable!("default policy must reject before calling the backend")
    }
}

#[tokio::test]
async fn initializes_lists_calls_and_closes_over_stdio_framing() {
    let backend = Arc::new(FakeBackend::default());
    let server = McpServer::new(backend.clone(), false);
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

    assert_eq!(tools.tools.len(), 10);
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
            .is_some_and(|text| text.text.contains("sending is disabled"))
    );
    assert_eq!(backend.send_calls.load(Ordering::SeqCst), 0);

    client.cancel().await.expect("client should close");
    server_task.await.expect("server task should join");
}
