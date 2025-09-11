use std::fmt;

/// Result type for ChatOps command execution
#[derive(Debug, Clone)]
pub enum ChatOpResult {
    /// Single message to send to chat
    Message(String),
    /// Multiple lines/blocks to send (paginated if needed)
    Block(Vec<String>),
    /// No output (silent success)
    #[allow(dead_code)]
    Silent,
    /// Formatted code block
    CodeBlock(String, Option<String>), // content, language
    /// Error message
    Error(String),
}

impl ChatOpResult {
    /// Convert to strings for chat output
    pub fn to_messages(&self) -> Vec<String> {
        match self {
            ChatOpResult::Message(msg) => vec![msg.clone()],
            ChatOpResult::Block(lines) => lines.clone(),
            ChatOpResult::Silent => vec![],
            ChatOpResult::CodeBlock(content, lang) => {
                let lang_str = lang.as_deref().unwrap_or("text");
                vec![format!("```{}\n{}\n```", lang_str, content)]
            }
            ChatOpResult::Error(err) => vec![format!("❌ Error: {}", err)],
        }
    }

    /// Check if result should be truncated for chat
    pub fn should_truncate(&self, max_lines: usize) -> bool {
        self.to_messages().len() > max_lines
    }

    /// Truncate result for chat output
    pub fn truncate(&self, max_lines: usize) -> ChatOpResult {
        let messages = self.to_messages();
        if messages.len() <= max_lines {
            return self.clone();
        }

        let message_count = messages.len();
        let mut truncated = messages.into_iter().take(max_lines - 1).collect::<Vec<_>>();
        truncated.push(format!(
            "... ({} more lines truncated)",
            message_count - max_lines + 1
        ));
        ChatOpResult::Block(truncated)
    }
}

/// Error type for ChatOps commands
#[derive(Debug, Clone)]
pub enum ChatOpError {
    /// Invalid command syntax
    InvalidSyntax(String),
    /// Missing required arguments
    MissingArguments(String),
    /// Permission denied
    PermissionDenied(String),
    /// External tool/service error
    #[allow(dead_code)]
    ExternalError(String),
    /// Network/connectivity error
    NetworkError(String),
    /// File system error
    #[allow(dead_code)]
    FileSystemError(String),
    /// Generic error with message
    Generic(String),
}

impl fmt::Display for ChatOpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatOpError::InvalidSyntax(msg) => write!(f, "Invalid syntax: {}", msg),
            ChatOpError::MissingArguments(msg) => write!(f, "Missing arguments: {}", msg),
            ChatOpError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ChatOpError::ExternalError(msg) => write!(f, "External error: {}", msg),
            ChatOpError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ChatOpError::FileSystemError(msg) => write!(f, "File system error: {}", msg),
            ChatOpError::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ChatOpError {}

impl From<ChatOpError> for ChatOpResult {
    fn from(error: ChatOpError) -> Self {
        ChatOpResult::Error(error.to_string())
    }
}
