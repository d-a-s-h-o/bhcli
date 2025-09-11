use crate::chatops::{ChatCommand, ChatOpError, ChatOpResult, CommandContext};
use std::process::Command;

/// ASCII art generation
pub struct AsciiCommand;

impl ChatCommand for AsciiCommand {
    fn name(&self) -> &'static str {
        "ascii"
    }
    fn description(&self) -> &'static str {
        "Generate ASCII art text"
    }
    fn usage(&self) -> &'static str {
        "/ascii <text>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify text to convert".to_string(),
            ));
        }

        let text = args.join(" ");

        // Try using figlet if available
        match Command::new("figlet").arg(&text).output() {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    Ok(ChatOpResult::CodeBlock(
                        result.to_string(),
                        Some("text".to_string()),
                    ))
                } else {
                    Ok(self.simple_ascii_art(&text))
                }
            }
            Err(_) => {
                // Fallback to simple ASCII art
                Ok(self.simple_ascii_art(&text))
            }
        }
    }
}

impl AsciiCommand {
    fn simple_ascii_art(&self, text: &str) -> ChatOpResult {
        // Simple block letters fallback
        let mut result = String::new();

        for ch in text.to_uppercase().chars() {
            match ch {
                'A' => result.push_str(" █████ \n██   ██\n███████\n██   ██\n██   ██\n"),
                'B' => result.push_str("██████ \n██   ██\n██████ \n██   ██\n██████ \n"),
                'C' => result.push_str(" ██████\n██     \n██     \n██     \n ██████\n"),
                'D' => result.push_str("██████ \n██   ██\n██   ██\n██   ██\n██████ \n"),
                'E' => result.push_str("███████\n██     \n█████  \n██     \n███████\n"),
                ' ' => result.push_str("       \n       \n       \n       \n       \n"),
                _ => result.push_str("██   ██\n██   ██\n██   ██\n██   ██\n██   ██\n"),
            }
        }

        ChatOpResult::CodeBlock(result, Some("text".to_string()))
    }
}

/// Fortune command
pub struct FortuneCommand;

impl ChatCommand for FortuneCommand {
    fn name(&self) -> &'static str {
        "fortune"
    }
    fn description(&self) -> &'static str {
        "Get a random fortune cookie"
    }
    fn usage(&self) -> &'static str {
        "/fortune"
    }

    fn execute(
        &self,
        _args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        match Command::new("fortune").output() {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    Ok(ChatOpResult::Message(format!("🥠 {}", result.trim())))
                } else {
                    Ok(self.fallback_fortune())
                }
            }
            Err(_) => Ok(self.fallback_fortune()),
        }
    }
}

impl FortuneCommand {
    fn fallback_fortune(&self) -> ChatOpResult {
        let fortunes = vec![
            "The best way to predict the future is to invent it.",
            "Programs must be written for people to read, and only incidentally for machines to execute.",
            "Any fool can write code that a computer can understand. Good programmers write code that humans can understand.",
            "First, solve the problem. Then, write the code.",
            "Experience is the name everyone gives to their mistakes.",
            "In order to be irreplaceable, one must always be different.",
            "Java is to JavaScript what car is to Carpet.",
            "There are only two hard things in Computer Science: cache invalidation and naming things.",
            "Code is like humor. When you have to explain it, it's bad.",
            "Programming today is a race between software engineers striving to build bigger and better idiot-proof programs, and the Universe trying to produce bigger and better idiots. So far, the Universe is winning.",
        ];

        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let fortune = fortunes.choose(&mut rng).unwrap_or(&fortunes[0]);

        ChatOpResult::Message(format!("🥠 {}", fortune))
    }
}

/// Message of the day
pub struct MotdCommand;

impl ChatCommand for MotdCommand {
    fn name(&self) -> &'static str {
        "motd"
    }
    fn description(&self) -> &'static str {
        "Show message of the day"
    }
    fn usage(&self) -> &'static str {
        "/motd"
    }

    fn execute(
        &self,
        _args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let motd = vec![
            "📢 **Message of the Day**".to_string(),
            "".to_string(),
            "🚀 Welcome to BHCLI with ChatOps!".to_string(),
            "💡 Type `/help` to see all available developer commands".to_string(),
            "🔧 Use `/commands` to list commands available to your role".to_string(),
            "🤖 Try `/explain <concept>` to learn about programming topics".to_string(),
            "📦 Check packages with `/crates`, `/npm`, or `/pip`".to_string(),
            "🌐 Test networks with `/ping`, `/dig`, or `/whois`".to_string(),
            "".to_string(),
            "Happy hacking! 🎯".to_string(),
        ];

        Ok(ChatOpResult::Block(motd))
    }
}

/// AFK (Away From Keyboard) status
pub struct AfkCommand;

impl ChatCommand for AfkCommand {
    fn name(&self) -> &'static str {
        "afk"
    }
    fn description(&self) -> &'static str {
        "Set yourself as away from keyboard"
    }
    fn usage(&self) -> &'static str {
        "/afk [message]"
    }

    fn execute(
        &self,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let message = if args.is_empty() {
            "Away from keyboard".to_string()
        } else {
            args.join(" ")
        };

        // In a real implementation, you'd store this in user state
        Ok(ChatOpResult::Message(format!(
            "💤 {} is now AFK: {}",
            context.username, message
        )))
    }
}

/// User alias management
pub struct AliasCommand;

impl ChatCommand for AliasCommand {
    fn name(&self) -> &'static str {
        "alias"
    }
    fn description(&self) -> &'static str {
        "Create personal command aliases"
    }
    fn usage(&self) -> &'static str {
        "/alias <name> <command> OR /alias list OR /alias remove <name>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify alias operation".to_string(),
            ));
        }

        match args[0].as_str() {
            "list" => {
                // In a real implementation, you'd load user's aliases from storage
                Ok(ChatOpResult::Message(format!(
                    "📝 Your aliases: (feature requires persistent storage implementation)"
                )))
            }
            "remove" | "rm" => {
                if args.len() < 2 {
                    return Err(ChatOpError::MissingArguments(
                        "Please specify alias name to remove".to_string(),
                    ));
                }
                let alias_name = &args[1];
                Ok(ChatOpResult::Message(format!(
                    "🗑️ Removed alias '{}' (feature requires persistent storage implementation)",
                    alias_name
                )))
            }
            _ => {
                if args.len() < 2 {
                    return Err(ChatOpError::MissingArguments(
                        "Please specify alias name and command".to_string(),
                    ));
                }
                let alias_name = &args[0];
                let command = args[1..].join(" ");

                Ok(ChatOpResult::Message(format!("✅ Created alias '{}' -> '{}' (feature requires persistent storage implementation)", alias_name, command)))
            }
        }
    }
}
