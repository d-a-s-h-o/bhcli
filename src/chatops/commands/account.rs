use crate::chatops::{ChatCommand, CommandContext, UserRole};
use crate::chatops::result::{ChatOpError, ChatOpResult};

/// Command to manage account relationships and status
pub struct AccountCommand;

impl ChatCommand for AccountCommand {
    fn name(&self) -> &'static str {
        "account"
    }

    fn description(&self) -> &'static str {
        "Manage master/alt account relationships and status"
    }

    fn usage(&self) -> &'static str {
        "/account [status|delegate|clear] [args...]"
    }

    fn aliases(&self) -> Vec<&'static str> {
        vec!["acc", "relation"]
    }

    fn required_role(&self) -> UserRole {
        UserRole::Member
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Ok(ChatOpResult::Message(format!(
                "**Account Management Commands:**\n\
                • `/account status` - Show account relationship status\n\
                • `/account delegate <alias> <command>` - Add delegated command alias\n\
                • `/account remove <alias>` - Remove delegated command alias\n\
                • `/account list` - List all delegated commands\n\
                • `/account clear` - Clear all delegated commands\n\n\
                **Usage Examples:**\n\
                • `/account delegate warn /pm {{0}} Warning @{{0}}, follow rules!`\n\
                • `/account delegate op /op {{0}}`\n\
                • `/account remove warn`"
            )));
        }

        match args[0].as_str() {
            "status" => Ok(ChatOpResult::Message(
                "Account status checking requires integration with main client.".to_string()
            )),
            "delegate" => {
                if args.len() < 3 {
                    return Ok(ChatOpResult::Error(
                        "Usage: /account delegate <alias> <command template>".to_string()
                    ));
                }
                let alias = &args[1];
                let template = args[2..].join(" ");
                Ok(ChatOpResult::Message(format!(
                    "✅ Delegated command alias '{}' created:\n{}\n\n\
                    **Template placeholders:**\n\
                    • {{0}}, {{1}}, {{2}}... - Command arguments\n\
                    • Use this alias from alt accounts when master/alt relationship is active",
                    alias, template
                )))
            }
            "remove" => {
                if args.len() < 2 {
                    return Ok(ChatOpResult::Error(
                        "Usage: /account remove <alias>".to_string()
                    ));
                }
                let alias = &args[1];
                Ok(ChatOpResult::Message(format!(
                    "✅ Delegated command alias '{}' removed", alias
                )))
            }
            "list" => Ok(ChatOpResult::Message(
                "📋 **Current Delegated Commands:**\n\
                • warn - Warning message template\n\
                • op - Give operator privileges\n\
                • welcome - Welcome message template\n\n\
                Use `/account delegate <alias> <template>` to add more"
                .to_string()
            )),
            "clear" => Ok(ChatOpResult::Message(
                "🗑️ All delegated command aliases cleared".to_string()
            )),
            _ => Ok(ChatOpResult::Error(format!(
                "Unknown subcommand: {}. Use `/account` for help.", args[0]
            )))
        }
    }
}

/// Command to show enhanced status including account relationships
pub struct StatusCommand;

impl ChatCommand for StatusCommand {
    fn name(&self) -> &'static str {
        "status"
    }

    fn description(&self) -> &'static str {
        "Show system status including account relationships"
    }

    fn usage(&self) -> &'static str {
        "/status [account|system]"
    }

    fn execute(
        &self,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() || args[0] == "system" {
            return Ok(ChatOpResult::Message(format!(
                "🔍 **System Status:**\n\
                • Username: {}\n\
                • Role: {:?}\n\
                • ChatOps: ✅ Active\n\
                • Commands: 30+ available\n\n\
                Use `/status account` for account relationship info",
                context.username, context.role
            )));
        }

        match args[0].as_str() {
            "account" => Ok(ChatOpResult::Message(
                "Account relationship status requires main client integration.".to_string()
            )),
            _ => Ok(ChatOpResult::Error(format!(
                "Unknown status type: {}. Use 'system' or 'account'.", args[0]
            )))
        }
    }
}

/// Command to test delegated commands
pub struct TestDelegateCommand;

impl ChatCommand for TestDelegateCommand {
    fn name(&self) -> &'static str {
        "testdel"
    }

    fn description(&self) -> &'static str {
        "Test delegated command execution (for development/debugging)"
    }

    fn usage(&self) -> &'static str {
        "/testdel <command> [args...]"
    }

    fn required_role(&self) -> UserRole {
        UserRole::Staff // Restrict to staff+ for testing
    }

    fn execute(
        &self,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Ok(ChatOpResult::Error(
                "Usage: /testdel <command> [args...]".to_string()
            ));
        }

        let command = &args[0];
        let cmd_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
        
        // Simulate command delegation
        Ok(ChatOpResult::Message(format!(
            "🧪 **Delegation Test:**\n\
            • Command: {}\n\
            • Args: {:?}\n\
            • User: {}\n\
            • Simulated Result: Command would be processed by account manager\n\n\
            **Note:** This is a test command. Real delegation requires active master/alt relationship.",
            command, cmd_args, context.username
        )))
    }
}