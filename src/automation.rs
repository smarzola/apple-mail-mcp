use std::{process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::Mutex,
    time::timeout,
};

use crate::{
    error::{MailError, Result},
    model::{
        Account, CheckMailRequest, CheckMailResult, CompositionResult, CreateDraftRequest,
        GetMessageRequest, ListMailboxesRequest, Mailbox, MessageDetail, MessageRef,
        MessageStateResult, MessageSummary, MoveMessageRequest, MoveMessageResult, SearchRequest,
        SendMessageRequest, SetMessageStateRequest,
    },
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_AUTOMATION_OUTPUT: usize = 1_048_576;
const MAX_AUTOMATION_ERROR: usize = 16_384;
const MAX_AUTOMATION_INPUT: usize = 524_288;

#[async_trait]
pub trait MailBackend: Send + Sync {
    async fn list_accounts(&self) -> Result<Vec<Account>>;
    async fn list_mailboxes(&self, request: ListMailboxesRequest) -> Result<Vec<Mailbox>>;
    async fn search_messages(&self, request: SearchRequest) -> Result<Vec<MessageSummary>>;
    async fn get_message(&self, request: GetMessageRequest) -> Result<MessageDetail>;
    async fn check_mail(&self, request: CheckMailRequest) -> Result<CheckMailResult>;
    async fn set_message_state(
        &self,
        request: SetMessageStateRequest,
    ) -> Result<MessageStateResult>;
    async fn move_message(&self, request: MoveMessageRequest) -> Result<MoveMessageResult>;
    async fn create_draft(&self, request: CreateDraftRequest) -> Result<CompositionResult>;
    async fn send_message(&self, request: SendMessageRequest) -> Result<CompositionResult>;
}

#[derive(Clone, Debug)]
pub struct JxaBackend {
    script: Arc<str>,
    timeout: Duration,
    gate: Arc<Mutex<()>>,
}

impl Default for JxaBackend {
    fn default() -> Self {
        Self {
            script: Arc::from(include_str!("../scripts/mail.js")),
            timeout: DEFAULT_TIMEOUT,
            gate: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(serde::Deserialize)]
struct ScriptEnvelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ScriptError>,
}

#[derive(serde::Deserialize)]
struct ScriptError {
    message: String,
}

struct CapturedOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl JxaBackend {
    async fn call<T, P>(&self, operation: &str, payload: &P) -> Result<T>
    where
        T: DeserializeOwned,
        P: Serialize + Sync,
    {
        if !cfg!(target_os = "macos") {
            return Err(MailError::AutomationUnavailable(
                "this backend requires macOS".to_owned(),
            ));
        }

        let payload = encode_payload(payload)?;
        let _guard = self.gate.lock().await;
        let mut child = Command::new("/usr/bin/osascript")
            .arg("-l")
            .arg("JavaScript")
            .arg("-e")
            .arg(self.script.as_ref())
            .arg(operation)
            .arg(payload)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let captured = match timeout(self.timeout, capture_output(&mut child)).await {
            Ok(Ok(captured)) => captured,
            Ok(Err(error)) => {
                terminate(&mut child).await;
                return Err(error);
            }
            Err(_) => {
                terminate(&mut child).await;
                return Err(MailError::AutomationTimeout(self.timeout));
            }
        };

        if !captured.status.success() {
            return Err(MailError::AutomationFailed(public_automation_message(
                &String::from_utf8_lossy(&captured.stderr),
            )));
        }

        let envelope: ScriptEnvelope<T> =
            serde_json::from_slice(&captured.stdout).map_err(|_| {
                MailError::InvalidResponse("response did not match the expected schema".to_owned())
            })?;
        if envelope.ok {
            envelope.data.ok_or_else(|| {
                MailError::InvalidResponse("successful response omitted data".to_owned())
            })
        } else {
            Err(MailError::AutomationFailed(public_automation_message(
                &envelope
                    .error
                    .map(|error| error.message)
                    .unwrap_or_default(),
            )))
        }
    }
}

fn encode_payload<P>(payload: &P) -> Result<String>
where
    P: Serialize + ?Sized,
{
    let payload = serde_json::to_string(payload)?;
    if payload.len() > MAX_AUTOMATION_INPUT {
        return Err(MailError::Validation(
            "serialized automation request exceeds the safety limit".to_owned(),
        ));
    }
    Ok(payload)
}

async fn capture_output(child: &mut Child) -> Result<CapturedOutput> {
    let stdout = child.stdout.take().ok_or_else(|| {
        MailError::InvalidResponse("automation stdout was not captured".to_owned())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        MailError::InvalidResponse("automation stderr was not captured".to_owned())
    })?;

    let stdout_future = read_bounded(stdout, MAX_AUTOMATION_OUTPUT);
    let stderr_future = read_bounded(stderr, MAX_AUTOMATION_ERROR);
    let status_future = child.wait();
    tokio::pin!(stdout_future, stderr_future, status_future);

    let mut stdout_data = None;
    let mut stderr_data = None;
    let mut status = None;
    loop {
        tokio::select! {
            result = &mut stdout_future, if stdout_data.is_none() => stdout_data = Some(result?),
            result = &mut stderr_future, if stderr_data.is_none() => stderr_data = Some(result?),
            result = &mut status_future, if status.is_none() => status = Some(result?),
        }
        if let (Some(status), Some(stdout), Some(stderr)) =
            (status, stdout_data.as_ref(), stderr_data.as_ref())
        {
            return Ok(CapturedOutput {
                status,
                stdout: stdout.clone(),
                stderr: stderr.clone(),
            });
        }
    }
}

async fn read_bounded<R>(mut reader: R, limit: usize) -> Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(count) > limit {
            return Err(MailError::OutputTooLarge);
        }
        output.extend_from_slice(&buffer[..count]);
    }
}

async fn terminate(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn public_automation_message(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("not authorized") || lower.contains("-1743") {
        "macOS denied Automation access; allow the controlling app under System Settings > Privacy & Security > Automation".to_owned()
    } else if lower.contains("application can't be found")
        || lower.contains("application isn’t running")
    {
        "Mail is unavailable; make sure Mail.app is installed and can be launched".to_owned()
    } else if lower.contains("account was not found") {
        "the selected Mail account was not found".to_owned()
    } else if lower.contains("mailbox path was not found") {
        "the selected mailbox path was not found".to_owned()
    } else if lower.contains("message was not found") {
        "the selected message was not found in that mailbox".to_owned()
    } else {
        "the automation request failed; verify Mail is running and Automation access is allowed"
            .to_owned()
    }
}

#[async_trait]
impl MailBackend for JxaBackend {
    async fn list_accounts(&self) -> Result<Vec<Account>> {
        self.call("list_accounts", &serde_json::Value::Null).await
    }

    async fn list_mailboxes(&self, request: ListMailboxesRequest) -> Result<Vec<Mailbox>> {
        self.call("list_mailboxes", &request).await
    }

    async fn search_messages(&self, request: SearchRequest) -> Result<Vec<MessageSummary>> {
        self.call("search_messages", &request).await
    }

    async fn get_message(&self, request: GetMessageRequest) -> Result<MessageDetail> {
        self.call("get_message", &request).await
    }

    async fn check_mail(&self, request: CheckMailRequest) -> Result<CheckMailResult> {
        self.call("check_mail", &request).await
    }

    async fn set_message_state(
        &self,
        request: SetMessageStateRequest,
    ) -> Result<MessageStateResult> {
        let result: MessageStateResult = self.call("set_message_state", &request).await?;
        if request.read.is_some_and(|expected| result.read != expected)
            || request
                .flagged
                .is_some_and(|expected| result.flagged != expected)
        {
            return Err(MailError::AutomationFailed(
                "Mail did not apply the requested message state".to_owned(),
            ));
        }
        Ok(result)
    }

    async fn move_message(&self, request: MoveMessageRequest) -> Result<MoveMessageResult> {
        let result: MoveMessageResult = self.call("move_message", &request).await?;
        if !result.moved || result.destination != request.destination {
            return Err(MailError::AutomationFailed(
                "Mail did not confirm the requested move".to_owned(),
            ));
        }
        Ok(result)
    }

    async fn create_draft(&self, request: CreateDraftRequest) -> Result<CompositionResult> {
        let result: CompositionResult = self.call("create_draft", &request).await?;
        if result.local_id <= 0 || result.sent || !result.visible {
            return Err(MailError::AutomationFailed(
                "Mail did not confirm draft creation".to_owned(),
            ));
        }
        Ok(result)
    }

    async fn send_message(&self, request: SendMessageRequest) -> Result<CompositionResult> {
        let result: CompositionResult = self.call("send_message", &request).await?;
        if result.local_id <= 0 || !result.sent {
            return Err(MailError::AutomationFailed(
                "Mail did not confirm that the message was sent".to_owned(),
            ));
        }
        Ok(result)
    }
}

pub fn message_ref(account_id: Option<String>, mailbox_path: Vec<String>, id: i64) -> MessageRef {
    MessageRef {
        account_id,
        mailbox_path,
        id,
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use assert_matches::assert_matches;

    use crate::{
        MailBackend,
        model::{
            CreateDraftRequest, MailboxRef, MessageRef, MoveMessageRequest, OutgoingMessage,
            SendMessageRequest, SetMessageStateRequest,
        },
    };

    use super::{
        JxaBackend, MAX_AUTOMATION_INPUT, MailError, encode_payload, public_automation_message,
    };

    fn backend(script: &str) -> JxaBackend {
        JxaBackend {
            script: Arc::from(script),
            timeout: Duration::from_secs(5),
            gate: Default::default(),
        }
    }

    #[test]
    fn public_error_never_replays_sensitive_input() {
        let raw = "person@example.com subject Secret mailbox Clients body private";
        let message = public_automation_message(raw);
        assert!(!message.contains("person@example.com"));
        assert!(!message.contains("Secret"));
        assert!(!message.contains("Clients"));
        assert!(!message.contains("private"));
    }

    #[tokio::test]
    async fn embedded_script_executes_without_a_runtime_script_path() {
        let value: serde_json::Value = JxaBackend::default()
            .call("__health", &serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(value["status"], "ready");
    }

    #[tokio::test]
    async fn automation_output_is_captured() {
        let script = r#"function run(argv) { return JSON.stringify({ok: true, data: {operation: argv[0]}}); }"#;
        let value: serde_json::Value = backend(script)
            .call("captured", &serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(value["operation"], "captured");
    }

    #[tokio::test]
    async fn error_envelope_is_classified_without_replaying_details() {
        let script = r#"function run() { return JSON.stringify({ok: false, error: {message: "person@example.com Secret body"}}); }"#;
        let error = backend(script)
            .call::<serde_json::Value, _>("failure", &serde_json::Value::Null)
            .await
            .unwrap_err();
        assert_matches!(error, MailError::AutomationFailed(message) if !message.contains("Secret") && !message.contains('@'));
    }

    #[tokio::test]
    async fn malformed_output_is_rejected() {
        let script = r#"function run() { return "not json"; }"#;
        assert_matches!(
            backend(script)
                .call::<serde_json::Value, _>("malformed", &serde_json::Value::Null)
                .await,
            Err(MailError::InvalidResponse(_))
        );
    }

    #[tokio::test]
    async fn nonzero_exit_is_classified() {
        let script = r#"function run() { throw new Error("private@example.com Secret"); }"#;
        let error = backend(script)
            .call::<serde_json::Value, _>("failure", &serde_json::Value::Null)
            .await
            .unwrap_err();
        assert_matches!(error, MailError::AutomationFailed(message) if !message.contains("Secret") && !message.contains('@'));
    }

    #[tokio::test]
    async fn timeout_terminates_the_script() {
        let script = r#"function run() { const end = Date.now() + 2000; while (Date.now() < end) {} return "done"; }"#;
        let mut backend = backend(script);
        backend.timeout = Duration::from_millis(50);
        assert_matches!(
            backend
                .call::<serde_json::Value, _>("slow", &serde_json::Value::Null)
                .await,
            Err(MailError::AutomationTimeout(_))
        );
    }

    #[tokio::test]
    async fn oversized_output_is_stopped_at_the_capture_limit() {
        let script = r#"function run() { return "x".repeat(1100000); }"#;
        assert_matches!(
            backend(script)
                .call::<serde_json::Value, _>("large", &serde_json::Value::Null)
                .await,
            Err(MailError::OutputTooLarge)
        );
    }

    #[tokio::test]
    async fn oversized_input_is_rejected_before_process_launch() {
        assert_matches!(
            JxaBackend::default()
                .call::<serde_json::Value, _>("__echo", &"x".repeat(600_000))
                .await,
            Err(MailError::Validation(_))
        );
    }

    #[test]
    fn serialized_input_limit_accepts_boundary_and_rejects_next_byte() {
        let at_limit = "x".repeat(MAX_AUTOMATION_INPUT - 2);
        assert_eq!(
            encode_payload(&at_limit).unwrap().len(),
            MAX_AUTOMATION_INPUT
        );
        let over_limit = "x".repeat(MAX_AUTOMATION_INPUT - 1);
        assert_matches!(encode_payload(&over_limit), Err(MailError::Validation(_)));
    }

    #[tokio::test]
    async fn embedded_truncation_preserves_astral_characters() {
        let value: serde_json::Value = JxaBackend::default()
            .call(
                "__test_truncate",
                &serde_json::json!({"text": "😀x", "limit": 1}),
            )
            .await
            .unwrap();
        assert_eq!(value["value"], "😀");
        assert_eq!(value["truncated"], true);
    }

    #[tokio::test]
    async fn caller_values_are_serialized_as_data_not_script_source() {
        let request = SendMessageRequest {
            message: OutgoingMessage {
                to: vec!["person@example.com".to_owned()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "\"; Application('Finder'); // 😀".to_owned(),
                body: "line one\nline two".to_owned(),
            },
            confirm: true,
        };
        let echoed: SendMessageRequest = JxaBackend::default()
            .call("__echo", &request)
            .await
            .unwrap();
        assert_eq!(echoed, request);
    }

    #[tokio::test]
    async fn derived_account_preserves_nested_mailbox_path_for_round_trip() {
        let reference: MessageRef = JxaBackend::default()
            .call(
                "__test_scope_reference",
                &serde_json::json!({
                    "account_id": "account-1",
                    "path": ["Projects", "Customer"],
                    "id": 42
                }),
            )
            .await
            .unwrap();

        assert_eq!(reference.account_id.as_deref(), Some("account-1"));
        assert_eq!(reference.mailbox_path, ["Projects", "Customer"]);
        assert_eq!(reference.id, 42);
    }

    #[tokio::test]
    async fn composition_validates_each_mail_confirmation_invariant() {
        let message = OutgoingMessage {
            to: vec!["person@example.com".to_owned()],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: String::new(),
            body: String::new(),
        };

        for data in [
            "{local_id: 0, sent: false, visible: true}",
            "{local_id: 1, sent: true, visible: true}",
            "{local_id: 1, sent: false, visible: false}",
        ] {
            let script =
                format!("function run() {{ return JSON.stringify({{ok: true, data: {data}}}); }}");
            assert_matches!(
                backend(&script)
                    .create_draft(CreateDraftRequest {
                        message: message.clone()
                    })
                    .await,
                Err(MailError::AutomationFailed(_))
            );
        }
        let script = r#"function run() { return JSON.stringify({ok: true, data: {local_id: 1, sent: false, visible: false}}); }"#;
        assert_matches!(
            backend(script)
                .send_message(SendMessageRequest {
                    message,
                    confirm: true
                })
                .await,
            Err(MailError::AutomationFailed(_))
        );
    }

    #[tokio::test]
    async fn state_mismatches_are_rejected_for_each_requested_field() {
        let script = r#"function run() { return JSON.stringify({ok: true, data: {message: {account_id: "a", mailbox_path: ["INBOX"], id: 1}, read: false, flagged: false}}); }"#;
        let message = MessageRef {
            account_id: Some("a".to_owned()),
            mailbox_path: vec!["INBOX".to_owned()],
            id: 1,
        };
        for request in [
            SetMessageStateRequest {
                message: message.clone(),
                read: Some(true),
                flagged: None,
            },
            SetMessageStateRequest {
                message: message.clone(),
                read: None,
                flagged: Some(true),
            },
        ] {
            assert_matches!(
                backend(script).set_message_state(request).await,
                Err(MailError::AutomationFailed(_))
            );
        }
    }

    #[tokio::test]
    async fn move_returns_only_confirmed_acknowledgement_not_a_synthetic_id() {
        let script = r#"function run() { return JSON.stringify({ok: true, data: {moved: true, destination: {account_id: "a", path: ["Archive"]}, message: {id: 999}}}); }"#;
        let result = backend(script)
            .move_message(MoveMessageRequest {
                message: MessageRef {
                    account_id: Some("a".to_owned()),
                    mailbox_path: vec!["INBOX".to_owned()],
                    id: 1,
                },
                destination: MailboxRef {
                    account_id: Some("a".to_owned()),
                    path: vec!["Archive".to_owned()],
                },
            })
            .await
            .unwrap();

        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["moved"], true);
        assert!(value.get("message").is_none());

        for data in [
            "{moved: false, destination: {account_id: \"a\", path: [\"Archive\"]}}",
            "{moved: true, destination: {account_id: \"b\", path: [\"Archive\"]}}",
        ] {
            let script =
                format!("function run() {{ return JSON.stringify({{ok: true, data: {data}}}); }}");
            assert_matches!(
                backend(&script)
                    .move_message(MoveMessageRequest {
                        message: MessageRef {
                            account_id: Some("a".to_owned()),
                            mailbox_path: vec!["INBOX".to_owned()],
                            id: 1,
                        },
                        destination: MailboxRef {
                            account_id: Some("a".to_owned()),
                            path: vec!["Archive".to_owned()],
                        },
                    })
                    .await,
                Err(MailError::AutomationFailed(_))
            );
        }
    }
}
