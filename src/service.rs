use crate::{
    MailBackend,
    error::{MailError, Result},
    model::{
        Account, CheckMailRequest, CheckMailResult, CompositionResult, CreateDraftRequest,
        GetMessageRequest, ListMailboxesRequest, MAX_BODY_CHARS, MAX_COMPOSE_BODY_CHARS,
        MAX_RECIPIENTS, MAX_RESULT_LIMIT, MAX_SUBJECT_CHARS, Mailbox, MailboxRef, MessageDetail,
        MessageRef, MessageStateResult, MessageSummary, MoveMessageRequest, MoveMessageResult,
        OutgoingMessage, SearchRequest, SendMessageRequest, SetMessageStateRequest,
    },
};

const MAX_SELECTOR_CHARS: usize = 256;
const MAX_MAILBOX_DEPTH: usize = 32;
const MAX_MAILBOX_PATH_CHARS: usize = 4096;

#[derive(Clone, Debug)]
pub struct MailService<B> {
    backend: B,
    send_enabled: bool,
}

impl<B> MailService<B>
where
    B: MailBackend,
{
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            send_enabled: false,
        }
    }

    pub fn with_send_enabled(mut self, enabled: bool) -> Self {
        self.send_enabled = enabled;
        self
    }

    pub async fn list_accounts(&self) -> Result<Vec<Account>> {
        self.backend.list_accounts().await
    }

    pub async fn list_mailboxes(&self, request: ListMailboxesRequest) -> Result<Vec<Mailbox>> {
        validate_optional_selector("account id", request.account_id.as_deref())?;
        self.backend.list_mailboxes(request).await
    }

    pub async fn search_messages(&self, request: SearchRequest) -> Result<Vec<MessageSummary>> {
        validate_mailbox(&request.mailbox)?;
        if request.limit == 0 || request.limit > MAX_RESULT_LIMIT {
            return Err(MailError::Validation(format!(
                "limit must be between 1 and {MAX_RESULT_LIMIT}"
            )));
        }
        validate_optional_selector("sender filter", request.sender_contains.as_deref())?;
        validate_optional_selector("subject filter", request.subject_contains.as_deref())?;
        self.backend.search_messages(request).await
    }

    pub async fn get_message(&self, request: GetMessageRequest) -> Result<MessageDetail> {
        validate_message(&request.message)?;
        if request.max_body_chars == 0 || request.max_body_chars > MAX_BODY_CHARS {
            return Err(MailError::Validation(format!(
                "max body characters must be between 1 and {MAX_BODY_CHARS}"
            )));
        }
        self.backend.get_message(request).await
    }

    pub async fn check_mail(&self, request: CheckMailRequest) -> Result<CheckMailResult> {
        validate_optional_selector("account id", request.account_id.as_deref())?;
        self.backend.check_mail(request).await
    }

    pub async fn set_message_state(
        &self,
        request: SetMessageStateRequest,
    ) -> Result<MessageStateResult> {
        validate_mutation_message(&request.message)?;
        if request.read.is_none() && request.flagged.is_none() {
            return Err(MailError::Validation(
                "at least one of read or flagged must be provided".to_owned(),
            ));
        }
        self.backend.set_message_state(request).await
    }

    pub async fn move_message(&self, mut request: MoveMessageRequest) -> Result<MoveMessageResult> {
        validate_mutation_message(&request.message)?;
        if request.destination.account_id.is_none() {
            request.destination.account_id = request.message.account_id.clone();
        }
        validate_mailbox(&request.destination)?;
        if request.message.mailbox() == request.destination {
            return Err(MailError::Validation(
                "source and destination mailboxes must differ".to_owned(),
            ));
        }
        self.backend.move_message(request).await
    }

    pub async fn create_draft(&self, request: CreateDraftRequest) -> Result<CompositionResult> {
        validate_outgoing(&request.message)?;
        self.backend.create_draft(request).await
    }

    pub async fn send_message(&self, request: SendMessageRequest) -> Result<CompositionResult> {
        validate_outgoing(&request.message)?;
        if !self.send_enabled {
            return Err(MailError::Validation(
                "sending is disabled by the current server policy".to_owned(),
            ));
        }
        if !request.confirm {
            return Err(MailError::Validation(
                "sending requires explicit confirmation".to_owned(),
            ));
        }
        self.backend.send_message(request).await
    }
}

fn validate_mutation_message(message: &MessageRef) -> Result<()> {
    validate_message(message)?;
    if message.account_id.is_none() {
        return Err(MailError::Validation(
            "message mutations require an account-scoped reference".to_owned(),
        ));
    }
    Ok(())
}

fn validate_outgoing(message: &OutgoingMessage) -> Result<()> {
    let recipient_count = message.to.len() + message.cc.len() + message.bcc.len();
    if recipient_count == 0 {
        return Err(MailError::Validation(
            "at least one recipient is required".to_owned(),
        ));
    }
    if recipient_count > MAX_RECIPIENTS {
        return Err(MailError::Validation(format!(
            "recipient count cannot exceed {MAX_RECIPIENTS}"
        )));
    }
    for address in message.to.iter().chain(&message.cc).chain(&message.bcc) {
        validate_address(address)?;
    }
    if message.subject.chars().count() > MAX_SUBJECT_CHARS {
        return Err(MailError::Validation(format!(
            "subject cannot exceed {MAX_SUBJECT_CHARS} characters"
        )));
    }
    if message.subject.chars().any(char::is_control) {
        return Err(MailError::Validation(
            "subject cannot contain control characters".to_owned(),
        ));
    }
    if message.body.chars().count() > MAX_COMPOSE_BODY_CHARS {
        return Err(MailError::Validation(format!(
            "body cannot exceed {MAX_COMPOSE_BODY_CHARS} characters"
        )));
    }
    if message.body.contains('\0') {
        return Err(MailError::Validation(
            "body cannot contain NUL characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_address(address: &str) -> Result<()> {
    if address != address.trim()
        || address.chars().count() > 254
        || address
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(MailError::Validation(
            "recipient address is not valid".to_owned(),
        ));
    }
    let Some((local, domain)) = address.split_once('@') else {
        return Err(MailError::Validation(
            "recipient address is not valid".to_owned(),
        ));
    };
    if local.is_empty() || local.chars().count() > 64 || domain.is_empty() || domain.contains('@') {
        return Err(MailError::Validation(
            "recipient address is not valid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_message(message: &MessageRef) -> Result<()> {
    if message.id <= 0 {
        return Err(MailError::Validation(
            "message id must be a positive integer".to_owned(),
        ));
    }
    validate_mailbox(&message.mailbox())
}

fn validate_mailbox(mailbox: &MailboxRef) -> Result<()> {
    validate_optional_selector("account id", mailbox.account_id.as_deref())?;
    if mailbox.path.is_empty() {
        return Err(MailError::Validation(
            "mailbox path must contain at least one component".to_owned(),
        ));
    }
    if mailbox.path.len() > MAX_MAILBOX_DEPTH {
        return Err(MailError::Validation(format!(
            "mailbox path cannot exceed {MAX_MAILBOX_DEPTH} components"
        )));
    }
    let total_chars = mailbox
        .path
        .iter()
        .map(|component| component.chars().count())
        .sum::<usize>();
    if total_chars > MAX_MAILBOX_PATH_CHARS {
        return Err(MailError::Validation(format!(
            "mailbox path cannot exceed {MAX_MAILBOX_PATH_CHARS} total characters"
        )));
    }
    for component in &mailbox.path {
        validate_selector("mailbox path component", component)?;
    }
    Ok(())
}

fn validate_optional_selector(name: &str, value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        validate_selector(name, value)?;
    }
    Ok(())
}

fn validate_selector(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(MailError::Validation(format!("{name} cannot be empty")));
    }
    if value.chars().count() > MAX_SELECTOR_CHARS {
        return Err(MailError::Validation(format!(
            "{name} cannot exceed {MAX_SELECTOR_CHARS} characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(MailError::Validation(format!(
            "{name} cannot contain control characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use assert_matches::assert_matches;
    use async_trait::async_trait;

    use super::*;

    #[derive(Clone, Default)]
    struct FakeBackend {
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl MailBackend for FakeBackend {
        async fn list_accounts(&self) -> Result<Vec<Account>> {
            self.calls.lock().unwrap().push("accounts");
            Ok(Vec::new())
        }

        async fn list_mailboxes(&self, _: ListMailboxesRequest) -> Result<Vec<Mailbox>> {
            self.calls.lock().unwrap().push("mailboxes");
            Ok(Vec::new())
        }

        async fn search_messages(&self, _: SearchRequest) -> Result<Vec<MessageSummary>> {
            self.calls.lock().unwrap().push("search");
            Ok(Vec::new())
        }

        async fn get_message(&self, _: GetMessageRequest) -> Result<MessageDetail> {
            self.calls.lock().unwrap().push("get");
            Ok(MessageDetail {
                summary: MessageSummary {
                    reference: MessageRef {
                        account_id: None,
                        mailbox_path: vec!["INBOX".to_owned()],
                        id: 1,
                    },
                    message_id: None,
                    subject: "Subject".to_owned(),
                    sender: "sender@example.com".to_owned(),
                    date_received: None,
                    read: false,
                    flagged: false,
                    size: 1,
                },
                content: "Body".to_owned(),
                content_truncated: false,
            })
        }

        async fn check_mail(&self, _: CheckMailRequest) -> Result<CheckMailResult> {
            self.calls.lock().unwrap().push("check");
            Ok(CheckMailResult { requested: true })
        }

        async fn set_message_state(
            &self,
            request: SetMessageStateRequest,
        ) -> Result<MessageStateResult> {
            self.calls.lock().unwrap().push("state");
            Ok(MessageStateResult {
                message: request.message,
                read: request.read.unwrap_or(false),
                flagged: request.flagged.unwrap_or(false),
            })
        }

        async fn move_message(&self, request: MoveMessageRequest) -> Result<MoveMessageResult> {
            self.calls.lock().unwrap().push("move");
            Ok(MoveMessageResult {
                moved: true,
                destination: request.destination,
            })
        }

        async fn create_draft(&self, _: CreateDraftRequest) -> Result<CompositionResult> {
            self.calls.lock().unwrap().push("draft");
            Ok(CompositionResult {
                local_id: 7,
                sent: false,
                visible: true,
            })
        }

        async fn send_message(&self, _: SendMessageRequest) -> Result<CompositionResult> {
            self.calls.lock().unwrap().push("send");
            Ok(CompositionResult {
                local_id: 8,
                sent: true,
                visible: false,
            })
        }
    }

    #[tokio::test]
    async fn invalid_limit_fails_before_backend() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let service = MailService::new(backend);
        let request = SearchRequest {
            limit: MAX_RESULT_LIMIT + 1,
            ..SearchRequest::default()
        };

        assert_matches!(
            service.search_messages(request).await,
            Err(MailError::Validation(_))
        );
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn valid_search_reaches_backend() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let service = MailService::new(backend);

        service
            .search_messages(SearchRequest::default())
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["search"]);
    }

    #[tokio::test]
    async fn all_invalid_bounds_and_selectors_fail_before_backend() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let service = MailService::new(backend);

        for limit in [0, MAX_RESULT_LIMIT + 1] {
            let request = SearchRequest {
                limit,
                ..SearchRequest::default()
            };
            assert_matches!(
                service.search_messages(request).await,
                Err(MailError::Validation(_))
            );
        }
        for filter in ["", "bad\nfilter"] {
            let request = SearchRequest {
                subject_contains: Some(filter.to_owned()),
                ..SearchRequest::default()
            };
            assert_matches!(
                service.search_messages(request).await,
                Err(MailError::Validation(_))
            );
        }
        let oversized = "x".repeat(MAX_SELECTOR_CHARS + 1);
        assert_matches!(
            service
                .list_mailboxes(ListMailboxesRequest {
                    account_id: Some(oversized),
                })
                .await,
            Err(MailError::Validation(_))
        );
        assert_matches!(
            service
                .search_messages(SearchRequest {
                    mailbox: MailboxRef {
                        account_id: None,
                        path: Vec::new(),
                    },
                    ..SearchRequest::default()
                })
                .await,
            Err(MailError::Validation(_))
        );
        for path in [
            vec!["x".to_owned(); MAX_MAILBOX_DEPTH + 1],
            vec!["x".repeat(MAX_SELECTOR_CHARS); 17],
        ] {
            assert_matches!(
                service
                    .search_messages(SearchRequest {
                        mailbox: MailboxRef {
                            account_id: None,
                            path,
                        },
                        ..SearchRequest::default()
                    })
                    .await,
                Err(MailError::Validation(_))
            );
        }
        assert_matches!(
            service
                .get_message(GetMessageRequest {
                    message: MessageRef {
                        account_id: None,
                        mailbox_path: vec!["INBOX".to_owned()],
                        id: 0,
                    },
                    max_body_chars: 1,
                })
                .await,
            Err(MailError::Validation(_))
        );
        for max_body_chars in [0, MAX_BODY_CHARS + 1] {
            assert_matches!(
                service
                    .get_message(GetMessageRequest {
                        message: MessageRef {
                            account_id: None,
                            mailbox_path: vec!["INBOX".to_owned()],
                            id: 1,
                        },
                        max_body_chars,
                    })
                    .await,
                Err(MailError::Validation(_))
            );
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn valid_read_operations_reach_the_backend() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let service = MailService::new(backend);

        service.list_accounts().await.unwrap();
        service
            .list_mailboxes(ListMailboxesRequest::default())
            .await
            .unwrap();
        service
            .get_message(GetMessageRequest {
                message: MessageRef {
                    account_id: None,
                    mailbox_path: vec!["INBOX".to_owned()],
                    id: 1,
                },
                max_body_chars: 10,
            })
            .await
            .unwrap();
        service
            .check_mail(CheckMailRequest::default())
            .await
            .unwrap();

        assert_eq!(
            *calls.lock().unwrap(),
            vec!["accounts", "mailboxes", "get", "check"]
        );
    }

    fn message(id: i64) -> MessageRef {
        MessageRef {
            account_id: Some("account-1".to_owned()),
            mailbox_path: vec!["INBOX".to_owned()],
            id,
        }
    }

    fn outgoing() -> OutgoingMessage {
        OutgoingMessage {
            to: vec!["person@example.com".to_owned()],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Hello".to_owned(),
            body: "Body".to_owned(),
        }
    }

    #[tokio::test]
    async fn invalid_mutations_fail_before_backend() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let service = MailService::new(backend);

        assert_matches!(
            service
                .set_message_state(SetMessageStateRequest {
                    message: message(1),
                    read: None,
                    flagged: None,
                })
                .await,
            Err(MailError::Validation(_))
        );
        let mut unscoped = message(1);
        unscoped.account_id = None;
        assert_matches!(
            service
                .set_message_state(SetMessageStateRequest {
                    message: unscoped,
                    read: Some(true),
                    flagged: None,
                })
                .await,
            Err(MailError::Validation(message)) if message.contains("account-scoped")
        );
        assert_matches!(
            service
                .move_message(MoveMessageRequest {
                    message: message(1),
                    destination: MailboxRef {
                        account_id: Some("account-1".to_owned()),
                        path: vec!["INBOX".to_owned()],
                    },
                })
                .await,
            Err(MailError::Validation(_))
        );
        assert_matches!(
            service
                .move_message(MoveMessageRequest {
                    message: message(1),
                    destination: MailboxRef {
                        account_id: None,
                        path: vec!["INBOX".to_owned()],
                    },
                })
                .await,
            Err(MailError::Validation(message)) if message.contains("differ")
        );

        let invalid_messages = [
            OutgoingMessage {
                to: Vec::new(),
                ..outgoing()
            },
            OutgoingMessage {
                to: vec!["not-an-address".to_owned()],
                ..outgoing()
            },
            OutgoingMessage {
                subject: "bad\nsubject".to_owned(),
                ..outgoing()
            },
            OutgoingMessage {
                body: "bad\0body".to_owned(),
                ..outgoing()
            },
            OutgoingMessage {
                to: vec!["person@example.com".to_owned(); MAX_RECIPIENTS + 1],
                ..outgoing()
            },
            OutgoingMessage {
                subject: "x".repeat(MAX_SUBJECT_CHARS + 1),
                ..outgoing()
            },
            OutgoingMessage {
                body: "x".repeat(MAX_COMPOSE_BODY_CHARS + 1),
                ..outgoing()
            },
        ];
        for message in invalid_messages {
            assert_matches!(
                service.create_draft(CreateDraftRequest { message }).await,
                Err(MailError::Validation(_))
            );
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn sending_requires_policy_and_per_call_confirmation() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let denied = MailService::new(backend.clone());
        assert_matches!(
            denied
                .send_message(SendMessageRequest {
                    message: outgoing(),
                    confirm: true,
                })
                .await,
            Err(MailError::Validation(message)) if message.contains("disabled")
        );

        let allowed = MailService::new(backend).with_send_enabled(true);
        assert_matches!(
            allowed
                .send_message(SendMessageRequest {
                    message: outgoing(),
                    confirm: false,
                })
                .await,
            Err(MailError::Validation(message)) if message.contains("confirmation")
        );
        allowed
            .send_message(SendMessageRequest {
                message: outgoing(),
                confirm: true,
            })
            .await
            .unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["send"]);
    }

    #[tokio::test]
    async fn valid_mutations_reach_backend() {
        let backend = FakeBackend::default();
        let calls = backend.calls.clone();
        let service = MailService::new(backend);

        service
            .set_message_state(SetMessageStateRequest {
                message: message(1),
                read: Some(true),
                flagged: Some(false),
            })
            .await
            .unwrap();
        service
            .move_message(MoveMessageRequest {
                message: message(1),
                destination: MailboxRef {
                    account_id: Some("account-1".to_owned()),
                    path: vec!["Archive".to_owned()],
                },
            })
            .await
            .unwrap();
        service
            .create_draft(CreateDraftRequest {
                message: outgoing(),
            })
            .await
            .unwrap();

        assert_eq!(*calls.lock().unwrap(), vec!["state", "move", "draft"]);
    }

    #[tokio::test]
    async fn move_inherits_and_dispatches_the_source_account() {
        let backend = FakeBackend::default();
        let service = MailService::new(backend);

        let result = service
            .move_message(MoveMessageRequest {
                message: message(1),
                destination: MailboxRef {
                    account_id: None,
                    path: vec!["Archive".to_owned()],
                },
            })
            .await
            .unwrap();

        assert!(result.moved);
        assert_eq!(result.destination.account_id.as_deref(), Some("account-1"));
        assert_eq!(result.destination.path, ["Archive"]);
    }
}
