use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DEFAULT_RESULT_LIMIT: u16 = 25;
pub const MAX_RESULT_LIMIT: u16 = 100;
pub const DEFAULT_BODY_CHARS: u32 = 16_384;
pub const MAX_BODY_CHARS: u32 = 65_536;
pub const DEFAULT_SNAPSHOT_LIMIT: u16 = 10;
pub const MAX_COMPOSE_BODY_CHARS: usize = 200_000;
pub const MAX_RECIPIENTS: usize = 100;
pub const MAX_SUBJECT_CHARS: usize = 998;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email_addresses: Vec<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct AccountList {
    pub accounts: Vec<Account>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MailboxRef {
    pub account_id: Option<String>,
    pub path: Vec<String>,
}

impl MailboxRef {
    pub fn inbox(account_id: Option<String>) -> Self {
        Self {
            account_id,
            path: vec!["INBOX".to_owned()],
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct Mailbox {
    pub reference: MailboxRef,
    pub name: String,
    pub unread_count: u64,
    pub unread_count_source: CountSource,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MailboxList {
    pub mailboxes: Vec<Mailbox>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MessageRef {
    pub account_id: Option<String>,
    pub mailbox_path: Vec<String>,
    pub id: i64,
}

impl MessageRef {
    pub fn mailbox(&self) -> MailboxRef {
        MailboxRef {
            account_id: self.account_id.clone(),
            path: self.mailbox_path.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MessageSummary {
    pub reference: MessageRef,
    pub message_id: Option<String>,
    pub subject: String,
    pub sender: String,
    pub date_received: Option<String>,
    pub read: bool,
    pub flagged: bool,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MessageSummaryList {
    pub messages: Vec<MessageSummary>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CountSource {
    MailReported,
    ExactBulkProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCompleteness {
    Complete,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MessageSearchResult {
    pub messages: Vec<MessageSummary>,
    pub scanned_count: u64,
    pub matched_count: u64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub completeness: SearchCompleteness,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MessageDetail {
    #[serde(flatten)]
    pub summary: MessageSummary,
    pub content: String,
    pub content_truncated: bool,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct ListMailboxesRequest {
    pub account_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct SearchRequest {
    pub mailbox: MailboxRef,
    pub unread: Option<bool>,
    pub flagged: Option<bool>,
    pub sender_contains: Option<String>,
    pub subject_contains: Option<String>,
    /// Inclusive RFC 3339 lower bound for the received date.
    pub received_after: Option<String>,
    /// Exclusive RFC 3339 upper bound for the received date.
    pub received_before: Option<String>,
    /// Opaque continuation cursor returned by a previous search.
    pub cursor: Option<String>,
    pub limit: u16,
}

impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            mailbox: MailboxRef::inbox(None),
            unread: None,
            flagged: None,
            sender_contains: None,
            subject_contains: None,
            received_after: None,
            received_before: None,
            cursor: None,
            limit: DEFAULT_RESULT_LIMIT,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct InboxSnapshotRequest {
    pub account_id: Option<String>,
    pub recent_limit: u16,
    pub unread_limit: u16,
}

impl Default for InboxSnapshotRequest {
    fn default() -> Self {
        Self {
            account_id: None,
            recent_limit: DEFAULT_SNAPSHOT_LIMIT,
            unread_limit: DEFAULT_SNAPSHOT_LIMIT,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct InboxSnapshot {
    pub mailbox: MailboxRef,
    pub total_count: u64,
    pub unread_count: u64,
    pub unread_count_source: CountSource,
    pub mail_reported_unread_count: u64,
    pub recent_messages: Vec<MessageSummary>,
    pub unread_messages: Vec<MessageSummary>,
    pub completeness: SearchCompleteness,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct GetMessageRequest {
    pub message: MessageRef,
    pub max_body_chars: u32,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct CheckMailRequest {
    pub account_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct CheckMailResult {
    pub requested: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct DoctorResult {
    pub platform: String,
    pub backend_ready: bool,
    pub account_count: Option<u64>,
    pub elapsed_ms: u64,
    pub diagnostic: Option<DoctorDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorDiagnostic {
    AutomationUnavailable,
    AutomationFailed,
    AutomationTimeout,
    OutputTooLarge,
    InvalidResponse,
    StartupFailed,
    BackendFailed,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct SetMessageStateRequest {
    pub message: MessageRef,
    pub read: Option<bool>,
    pub flagged: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MessageStateResult {
    pub message: MessageRef,
    pub read: bool,
    pub flagged: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MoveMessageRequest {
    pub message: MessageRef,
    pub destination: MailboxRef,
    pub confirm: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct MoveMessageResult {
    pub moved: bool,
    pub destination: MailboxRef,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct OutgoingMessage {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct CreateDraftRequest {
    pub message: OutgoingMessage,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct CreateReplyDraftRequest {
    pub message: MessageRef,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct ReplyDraftResult {
    pub source: MessageRef,
    pub local_id: i64,
    pub subject: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub sent: bool,
    pub draft_present: bool,
    pub visible: bool,
    pub content_verified: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct SendMessageRequest {
    pub message: OutgoingMessage,
    pub confirm: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
pub struct CompositionResult {
    pub local_id: i64,
    pub sent: bool,
    pub visible: bool,
}
