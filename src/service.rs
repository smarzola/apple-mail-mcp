use crate::{
    MailBackend,
    error::{MailError, Result},
    model::{
        Account, CheckMailRequest, CheckMailResult, GetMessageRequest, ListMailboxesRequest,
        MAX_BODY_CHARS, MAX_RESULT_LIMIT, Mailbox, MailboxRef, MessageDetail, MessageRef,
        MessageSummary, SearchRequest,
    },
};

const MAX_SELECTOR_CHARS: usize = 256;

#[derive(Clone, Debug)]
pub struct MailService<B> {
    backend: B,
}

impl<B> MailService<B>
where
    B: MailBackend,
{
    pub fn new(backend: B) -> Self {
        Self { backend }
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
}
