pub mod automation;
pub mod cli;
pub mod error;
pub mod model;
pub mod service;

pub use automation::{JxaBackend, MailBackend};
pub use error::{MailError, Result};
pub use service::MailService;
