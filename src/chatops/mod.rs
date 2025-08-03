//! ChatOps - Developer-focused slash commands for BHCLI
//! 
//! This module provides a flexible and extensible slash command system 
//! to support advanced developer-focused features.

pub mod command_router;
pub mod commands;
pub mod registry;
pub mod result;

pub use command_router::ChatOpsRouter;
pub use registry::CommandRegistry;
pub use result::{ChatOpResult, ChatOpError};

/// Context provided to commands during execution
#[derive(Clone)]
pub struct CommandContext {
    pub username: String,
    #[allow(dead_code)]
    pub room: Option<String>,
    pub role: UserRole,
}

/// User role for permission checking
#[derive(Clone, Debug, PartialEq)]
pub enum UserRole {
    Guest,
    Member,
    Staff,
    Admin,
}

/// Main ChatOps handler trait
pub trait ChatCommand: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn usage(&self) -> &'static str;
    fn execute(&self, args: Vec<String>, context: &CommandContext) -> Result<ChatOpResult, ChatOpError>;
    fn aliases(&self) -> Vec<&'static str> { vec![] }
    fn required_role(&self) -> UserRole { UserRole::Guest }
}

/// Initialize the ChatOps system with default commands
pub fn init_chatops() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    
    // Documentation & Lookup
    registry.register(Box::new(commands::doc::ManCommand));
    registry.register(Box::new(commands::doc::DocCommand));
    registry.register(Box::new(commands::doc::ExplainCommand));
    registry.register(Box::new(commands::doc::CheatCommand));
    registry.register(Box::new(commands::doc::StackOverflowCommand));
    registry.register(Box::new(commands::doc::RefCommand));
    
    // Tooling & Utilities
    registry.register(Box::new(commands::tools::HashCommand));
    registry.register(Box::new(commands::tools::UuidCommand));
    registry.register(Box::new(commands::tools::Base64Command));
    registry.register(Box::new(commands::tools::RegexCommand));
    registry.register(Box::new(commands::tools::WhoisCommand));
    registry.register(Box::new(commands::tools::DigCommand));
    registry.register(Box::new(commands::tools::IpInfoCommand));
    registry.register(Box::new(commands::tools::RandCommand));
    registry.register(Box::new(commands::tools::TimeCommand));
    
    // Chat Linking & Session Intelligence
    registry.register(Box::new(commands::chat::ChatLinkCommand));
    registry.register(Box::new(commands::chat::QuoteCommand));
    registry.register(Box::new(commands::chat::RoomsCommand));
    registry.register(Box::new(commands::chat::WhereIsCommand));
    
    // AI Integration (if available)
    registry.register(Box::new(commands::ai::SummarizeCommand));
    registry.register(Box::new(commands::ai::TranslateCommand));
    registry.register(Box::new(commands::ai::FixCommand));
    registry.register(Box::new(commands::ai::ReviewCommand));
    
    // GitHub and Git Integration
    registry.register(Box::new(commands::github::GitHubCommand));
    registry.register(Box::new(commands::github::GistCommand));
    registry.register(Box::new(commands::github::CratesCommand));
    registry.register(Box::new(commands::github::NpmCommand));
    registry.register(Box::new(commands::github::PipCommand));
    
    // Network & Protocol Diagnostics
    registry.register(Box::new(commands::network::PingCommand));
    registry.register(Box::new(commands::network::TraceCommand));
    registry.register(Box::new(commands::network::PortScanCommand));
    registry.register(Box::new(commands::network::HeadersCommand));
    registry.register(Box::new(commands::network::CurlCommand));
    registry.register(Box::new(commands::network::SslCommand));
    registry.register(Box::new(commands::network::TorCheckCommand));
    
    // Fun & Misc
    registry.register(Box::new(commands::misc::AsciiCommand));
    registry.register(Box::new(commands::misc::FortuneCommand));
    registry.register(Box::new(commands::misc::MotdCommand));
    registry.register(Box::new(commands::misc::AfkCommand));
    registry.register(Box::new(commands::misc::AliasCommand));
    
    registry
}
