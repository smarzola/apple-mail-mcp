use serde::{Deserialize, Serialize};

pub const DEFAULT_RESULT_LIMIT: u16 = 25;
pub const MAX_RESULT_LIMIT: u16 = 100;
pub const DEFAULT_BODY_CHARS: u32 = 16_384;
pub const MAX_BODY_CHARS: u32 = 65_536;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email_addresses: Vec<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Mailbox {
    pub reference: MailboxRef,
    pub name: String,
    pub unread_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct MessageDetail {
    #[serde(flatten)]
    pub summary: MessageSummary,
    pub content: String,
    pub content_truncated: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct ListMailboxesRequest {
    pub account_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SearchRequest {
    pub mailbox: MailboxRef,
    pub unread: Option<bool>,
    pub flagged: Option<bool>,
    pub sender_contains: Option<String>,
    pub subject_contains: Option<String>,
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
            limit: DEFAULT_RESULT_LIMIT,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GetMessageRequest {
    pub message: MessageRef,
    pub max_body_chars: u32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct CheckMailRequest {
    pub account_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CheckMailResult {
    pub requested: bool,
}
