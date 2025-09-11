use crate::ai_service::AIService;
use crate::chatops::{ChatOpResult, CommandContext, CommandRegistry, UserRole};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Main router for handling ChatOps commands
pub struct ChatOpsRouter {
    registry: CommandRegistry,
}

impl ChatOpsRouter {
    pub fn new() -> Self {
        Self {
            registry: crate::chatops::init_chatops(),
        }
    }

    pub fn new_with_ai(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            registry: crate::chatops::init_chatops_with_ai(ai_service, runtime),
        }
    }

    /// Process a slash command input
    pub fn process_command(
        &self,
        input: &str,
        username: &str,
        role: UserRole,
    ) -> Option<ChatOpResult> {
        // Skip if not a chatops command (let existing system handle it)
        if !self.is_chatops_command(input) {
            return None;
        }

        let parts = self.parse_command(input);
        if parts.is_empty() {
            return Some(ChatOpResult::Error("Empty command".to_string()));
        }

        let command_name = &parts[0];
        let args = parts[1..].to_vec();

        // Handle special built-in commands
        match command_name.as_str() {
            "chatops" => return Some(self.handle_help_command(vec![], &role)),
            "commands" | "list" => return Some(self.handle_list_commands(&role)),
            "help" => {
                // Only handle specific chatops help, let general help fall through
                if !args.is_empty() && self.registry.get_help(&args[0]).is_some() {
                    return Some(self.handle_help_command(args, &role));
                }
                // Let general /help fall through to main help system
                return None;
            }
            _ => {}
        }

        let context = CommandContext {
            username: username.to_string(),
            room: None, // TODO: Extract from app state if needed
            role,
        };

        match self.registry.execute_command(command_name, args, &context) {
            Ok(result) => {
                // Truncate long results to prevent chat spam but allow more context
                let truncated = if result.should_truncate(40) {
                    result.truncate(40)
                } else {
                    result
                };
                Some(truncated)
            }
            Err(error) => Some(ChatOpResult::Error(error.to_string())),
        }
    }

    /// Check if this is a chatops command (vs existing system command)
    fn is_chatops_command(&self, input: &str) -> bool {
        if !input.starts_with('/') {
            return false;
        }

        let parts = self.parse_command(input);
        if parts.is_empty() {
            return false;
        }

        let command_name = &parts[0];

        // Built-in chatops commands
        if matches!(command_name.as_str(), "help" | "commands" | "list") {
            return true;
        }

        // Check if it's a registered chatops command
        self.registry.get_command(command_name).is_some()
    }

    /// Parse command input into parts
    fn parse_command(&self, input: &str) -> Vec<String> {
        let input = input.trim_start_matches('/');

        // Simple parsing - split on whitespace but respect quotes
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '"' if !in_quotes => {
                    in_quotes = true;
                }
                '"' if in_quotes => {
                    in_quotes = false;
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                ' ' | '\t' if !in_quotes => {
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts
    }

    /// Handle help command
    fn handle_help_command(&self, args: Vec<String>, _role: &UserRole) -> ChatOpResult {
        if args.is_empty() {
            // General ChatOps help - return as single message for better full-screen display
            let help_text = vec![
                "🤖 **ChatOps Developer Commands Available:**",
                "",
                "**Documentation & Lookup:**",
                "/man <command> - Manual pages",
                "/doc <lang> <term> - Language docs",
                "/explain <topic> - Explain concepts",
                "/cheat <term> - Cheat sheets",
                "/so <query> - StackOverflow search",
                "",
                "**Tools & Utilities:**",
                "/hash <algo> <text> - Hash functions",
                "/uuid - Generate UUID",
                "/base64 <encode|decode> <text>",
                "/regex <pattern> <text> - Test regex",
                "/time - Current time",
                "",
                "**Network Tools:**",
                "/whois <domain> - Domain lookup",
                "/dig <domain> - DNS lookup",
                "/ping <host> - Ping host",
                "/headers <url> - HTTP headers",
                "",
                "**GitHub Integration:**",
                "/github <user>/<repo> - Repo info",
                "/crates <crate> - Rust crate info",
                "/npm <package> - NPM package info",
                "",
                "Use `/help <command>` for help on specific ChatOps commands.",
                "Use `/commands` to list available commands for your role.",
                "Use `/help` for general chat commands and shortcuts.",
            ]
            .join("\n");

            ChatOpResult::Message(help_text)
        } else {
            // Specific command help
            let command_name = &args[0];
            match self.registry.get_help(command_name) {
                Some(help) => ChatOpResult::Message(help),
                None => {
                    ChatOpResult::Error(format!("No help available for command: {}", command_name))
                }
            }
        }
    }

    /// Handle list commands
    fn handle_list_commands(&self, role: &UserRole) -> ChatOpResult {
        let commands = self.registry.list_commands(role);
        if commands.is_empty() {
            return ChatOpResult::Message("No commands available for your role.".to_string());
        }

        let mut output = vec![
            format!("📋 **Available Commands ({} total):**", commands.len()),
            "".to_string(),
        ];

        for (name, description) in commands {
            output.push(format!("**{}** - {}", name, description));
        }

        ChatOpResult::Block(output)
    }

    /// Register a user alias
    #[allow(dead_code)]
    pub fn register_user_alias(&mut self, alias: String, target: String) {
        self.registry.register_alias(alias, target);
    }

    /// Remove a user alias
    #[allow(dead_code)]
    pub fn remove_user_alias(&mut self, alias: &str) {
        self.registry.remove_alias(alias);
    }
}
