use crate::ai_service::AIService;
use crate::{PostType, Users};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Timelike, Utc};
use crossbeam_channel::Sender;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;

/// Represents a chat message stored by the bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotChatMessage {
    pub id: Option<u64>,
    #[serde(with = "datetime_format")]
    pub timestamp: DateTime<Utc>,
    pub username: String,
    pub content: String,
    pub message_type: MessageType,
    pub is_deleted: bool,
    #[serde(with = "datetime_option_format")]
    pub deleted_at: Option<DateTime<Utc>>,
    pub edit_history: Vec<String>,
}

/// Types of messages the bot can track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Normal,
    PrivateMessage { to: String },
    System,
    Join,
    Leave,
    Kick { by: String, reason: Option<String> },
    Ban { by: String, reason: Option<String> },
}

/// User statistics tracked by the bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub username: String,
    #[serde(with = "datetime_format")]
    pub first_seen: DateTime<Utc>,
    #[serde(with = "datetime_format")]
    pub last_seen: DateTime<Utc>,
    pub total_messages: u64,
    #[serde(with = "duration_secs")]
    pub total_time_online: Duration,
    pub kicks_received: u64,
    pub kicks_given: u64,
    pub bans_received: u64,
    pub bans_given: u64,
    pub warnings_received: u64,
    pub warnings_given: u64,
    pub session_starts: u64,
    pub favorite_words: HashMap<String, u64>,
    pub hourly_activity: [u64; 24], // Activity by hour of day
    pub daily_activity: HashMap<String, u64>, // Activity by date
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

mod datetime_format {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(datetime: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        datetime.timestamp().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp = i64::deserialize(deserializer)?;
        DateTime::from_timestamp(timestamp, 0)
            .ok_or_else(|| serde::de::Error::custom("invalid timestamp"))
    }
}

mod datetime_option_format {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(datetime: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match datetime {
            Some(dt) => Some(dt.timestamp()).serialize(serializer),
            None => None::<i64>.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<i64>::deserialize(deserializer)? {
            Some(timestamp) => Ok(DateTime::from_timestamp(timestamp, 0)),
            None => Ok(None),
        }
    }
}

impl Default for UserStats {
    fn default() -> Self {
        Self {
            username: String::new(),
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            total_messages: 0,
            total_time_online: Duration::new(0, 0),
            kicks_received: 0,
            kicks_given: 0,
            bans_received: 0,
            bans_given: 0,
            warnings_received: 0,
            warnings_given: 0,
            session_starts: 0,
            favorite_words: HashMap::new(),
            hourly_activity: [0; 24],
            daily_activity: HashMap::new(),
        }
    }
}

/// Bot command structure
#[derive(Debug, Clone)]
pub struct BotCommand {
    pub name: String,
    pub args: Vec<String>,
    pub requester: String,
    pub channel_context: Option<String>, // "members" for [M] channel, None for public
    pub is_member: bool,                 // True if requester is a member
}

/// Bot response types
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BotResponse {
    PublicMessage(String),
    PrivateMessage { to: String, content: String },
    Action(BotAction),
    Error(String),
}

/// Actions the bot can perform
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BotAction {
    Kick { username: String, reason: String },
    Ban { username: String, reason: String },
    Warn { username: String, message: String },
    SaveChatLog { filename: String },
    RestoreMessage { message_id: u64 },
}

/// Configuration for the bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub bot_name: String,
    pub data_directory: PathBuf,
    pub max_message_history: usize,
    pub auto_save_interval: Duration,
    pub enable_ai_integration: bool,
    pub admin_users: Vec<String>,
    pub moderator_users: Vec<String>,
    pub command_prefix: String,
    pub respond_to_mentions: bool,
    pub log_private_messages: bool,
    pub max_export_lines: usize,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            bot_name: "BotAssistant".to_string(),
            data_directory: PathBuf::from("bot_data"),
            max_message_history: 50000,
            auto_save_interval: Duration::from_secs(300), // 5 minutes
            enable_ai_integration: true,
            admin_users: Vec::new(),
            moderator_users: Vec::new(),
            command_prefix: "!".to_string(),
            respond_to_mentions: true,
            log_private_messages: false,
            max_export_lines: 10000,
        }
    }
}

/// Main bot system
pub struct BotSystem {
    config: BotConfig,
    message_history: Arc<Mutex<Vec<BotChatMessage>>>,
    user_stats: Arc<Mutex<HashMap<String, UserStats>>>,
    current_users: Arc<Mutex<Users>>,
    ai_service: Option<Arc<AIService>>,

    tx: Sender<PostType>,
    running: Arc<Mutex<bool>>,
    last_save: Arc<Mutex<SystemTime>>,
}

impl BotSystem {
    /// Create a new bot system
    pub fn new(
        config: BotConfig,
        tx: Sender<PostType>,
        ai_service: Option<Arc<AIService>>,
        _runtime: Option<Arc<Runtime>>,
    ) -> Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all(&config.data_directory)?;

        let bot = Self {
            config,
            message_history: Arc::new(Mutex::new(Vec::new())),
            user_stats: Arc::new(Mutex::new(HashMap::new())),
            current_users: Arc::new(Mutex::new(Users::default())),
            ai_service,
            tx,
            running: Arc::new(Mutex::new(false)),
            last_save: Arc::new(Mutex::new(SystemTime::now())),
        };

        // Load existing data
        bot.load_data()?;

        Ok(bot)
    }

    /// Start the bot system
    pub fn start(&self) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                return Err(anyhow!("Bot system is already running"));
            }
            *running = true;
        }

        info!("Starting bot system: {}", self.config.bot_name);

        // Start auto-save thread
        self.start_auto_save_thread();

        // Send startup message (non-blocking)
        let startup_msg = format!(
            "🤖 {} is now online! Type @{} help for available commands.",
            self.config.bot_name, self.config.bot_name
        );
        if let Err(e) = self
            .tx
            .try_send(PostType::Post(startup_msg, Some("0".to_string())))
        {
            warn!(
                "Could not send startup message (channel may be disconnected): {}",
                e
            );
        }

        Ok(())
    }

    /// Stop the bot system
    pub fn stop(&self) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            if !*running {
                return Ok(());
            }
            *running = false;
        }

        info!("Stopping bot system: {}", self.config.bot_name);

        // Save all data before stopping
        self.save_data()?;

        // Try to send shutdown message, but don't fail if channel is disconnected
        let shutdown_msg = format!("🤖 {} is going offline. Data saved.", self.config.bot_name);
        if let Err(e) = self
            .tx
            .try_send(PostType::Post(shutdown_msg, Some("0".to_string())))
        {
            // Log the error but don't fail the shutdown process
            warn!(
                "Could not send shutdown message (channel may be disconnected): {}",
                e
            );
        }

        Ok(())
    }

    /// Process a new message
    pub fn process_message(
        &self,
        username: &str,
        content: &str,
        message_type: MessageType,
        message_id: Option<u64>,
        channel_context: Option<&str>, // "members" for [M] channel, None for public
        is_member: bool,               // True if requester is a member
    ) -> Result<()> {
        let timestamp = Utc::now();

        // Create bot message record
        let bot_message = BotChatMessage {
            id: message_id,
            timestamp,
            username: username.to_string(),
            content: content.to_string(),
            message_type: message_type.clone(),
            is_deleted: false,
            deleted_at: None,
            edit_history: Vec::new(),
        };

        // Store message in history
        {
            let mut history = self.message_history.lock().unwrap();
            history.push(bot_message);

            // Limit history size
            if history.len() > self.config.max_message_history {
                let excess = history.len() - self.config.max_message_history;
                history.drain(0..excess);
            }
        }

        // Update user statistics
        self.update_user_stats(username, content, &message_type, timestamp)?;

        // Check for bot commands if mentioned and command is at start of message
        if self.config.respond_to_mentions && self.is_bot_mentioned_at_start(content) {
            info!(
                "Bot '{}' processing command from {}: {}",
                self.config.bot_name, username, content
            );
            self.handle_mention_commands(
                username,
                content,
                matches!(message_type, MessageType::PrivateMessage { .. }),
                channel_context,
                is_member,
            )?;
        }

        Ok(())
    }

    /// Process message deletion
    #[allow(dead_code)]
    pub fn process_message_deletion(&self, message_id: u64) -> Result<()> {
        let mut history = self.message_history.lock().unwrap();

        if let Some(message) = history.iter_mut().find(|m| m.id == Some(message_id)) {
            message.is_deleted = true;
            message.deleted_at = Some(Utc::now());

            info!("Bot recorded message deletion: ID {}", message_id);
        }

        Ok(())
    }

    /// Update current users list
    #[allow(dead_code)]
    pub fn update_users(&self, users: Users) -> Result<()> {
        *self.current_users.lock().unwrap() = users;
        Ok(())
    }

    /// Check if bot is mentioned at the start of content (not embedded)
    fn is_bot_mentioned_at_start(&self, content: &str) -> bool {
        let mention_pattern = format!("@{}", self.config.bot_name.to_lowercase());
        let binding = content.to_lowercase();
        let content_lower = binding.trim();
        content_lower.starts_with(&mention_pattern)
    }

    /// Handle commands when bot is mentioned
    fn handle_mention_commands(
        &self,
        requester: &str,
        content: &str,
        is_private: bool,
        channel_context: Option<&str>,
        is_member: bool,
    ) -> Result<()> {
        let commands =
            self.parse_commands(content, requester, is_private, channel_context, is_member)?;

        for command in commands {
            info!(
                "Bot '{}' executing command: '{}'",
                self.config.bot_name, command.name
            );
            match self.execute_command(&command) {
                Ok(response) => {
                    self.send_response_with_context(response, &command)?;
                }
                Err(e) => {
                    warn!(
                        "Bot '{}' command execution failed: {} - {}",
                        self.config.bot_name, command.name, e
                    );
                    let error_response = BotResponse::PrivateMessage {
                        to: requester.to_string(),
                        content: format!("Error executing {}: {}", command.name, e),
                    };
                    self.send_response_with_context(error_response, &command)?;
                }
            }
        }

        Ok(())
    }

    /// Parse commands from message content
    fn parse_commands(
        &self,
        content: &str,
        requester: &str,
        _is_private: bool,
        channel_context: Option<&str>,
        is_member: bool,
    ) -> Result<Vec<BotCommand>> {
        let mut commands = Vec::new();

        // Look for commands in the format: @botname command arg1 arg2 (at start of message only)
        let mention_pattern = format!("@{}", self.config.bot_name.to_lowercase());
        let binding = content.to_lowercase();
        let content_lower = binding.trim();

        if content_lower.starts_with(&mention_pattern) {
            let after_mention = &content[mention_pattern.len()..];
            let words: Vec<&str> = after_mention.split_whitespace().collect();

            if !words.is_empty() {
                let command_name = words[0].to_string();
                let args: Vec<String> = words[1..].iter().map(|&s| s.to_string()).collect();

                commands.push(BotCommand {
                    name: command_name,
                    args,
                    requester: requester.to_string(),
                    channel_context: channel_context.map(|s| s.to_string()),
                    is_member,
                });
            }
        }

        Ok(commands)
    }

    /// Execute a bot command
    fn execute_command(&self, command: &BotCommand) -> Result<BotResponse> {
        // Check permissions for moderation commands
        match command.name.to_lowercase().as_str() {
            "kick" | "ban" if !command.is_member => {
                return Ok(BotResponse::PrivateMessage {
                    to: command.requester.clone(),
                    content: "❌ Only members can use moderation commands".to_string(),
                });
            }
            _ => {}
        }

        match command.name.to_lowercase().as_str() {
            "help" => self.cmd_help(command),
            "stats" => self.cmd_stats(command),
            "recall" => self.cmd_recall(command),
            "search" => self.cmd_search(command),
            "export" => self.cmd_export(command),
            "restore" => self.cmd_restore(command),
            "users" => self.cmd_users(command),
            "top" => self.cmd_top(command),
            "history" => self.cmd_history(command),
            "summary" => self.cmd_summary(command),
            "status" => self.cmd_status(command),
            "purge" => self.cmd_purge(command),
            "kick" => self.cmd_kick(command),
            "ban" => self.cmd_ban(command),
            _ => Err(anyhow!("Unknown command: {}", command.name)),
        }
    }

    /// Help command
    fn cmd_help(&self, _command: &BotCommand) -> Result<BotResponse> {
        // Get AI status for help message
        let ai_status_note = if self.ai_service.is_some() {
            "\n\n🤖 **AI Features:**\nAdvanced AI commands are available via ChatOps (type `/help` in main chat).\nIf AI features are unavailable, it may be due to API quota limits."
        } else {
            "\n\n🤖 **AI Features:**\nAI integration is disabled. Advanced AI commands are not available."
        };

        let help_text = format!(
            "🤖 **{} Commands:**\n\n\
            **📊 Statistics & Info:**\n\
            • `@{} stats [username]` - View user statistics\n\
            • `@{} users` - List current online users\n\
            • `@{} top [messages|time|kicks]` - Top user rankings\n\
            • `@{} status` - Bot system status\n\n\
            **🔍 Search & Recall:**\n\
            • `@{} recall <timestamp>` - Find message by timestamp\n\
            • `@{} search <term>` - Search message history\n\
            • `@{} history <username> [count]` - User message history\n\n\
            **📋 Data Management:**\n\
            • `@{} export [username] [days]` - Export chat logs\n\
            • `@{} restore <message_id>` - Restore deleted message\n\
            • `@{} summary [hours]` - Chat activity summary\n\n\
            **🛠️ Admin Commands:**\n\
            • `@{} purge <username>` - Clear user data (admin only)\n\n\
            **⚖️ Moderation Commands (Members Only):**\n\
            • `@{} kick <username> [reason]` - Kick user from chat\n\
            • `@{} ban <username> [reason]` - Ban user from chat\n\n\
            ℹ️  **Note:** These core commands always work, even when AI services are unavailable.\n\
            Use `@{} help <command>` for detailed help on specific commands.{}",
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            self.config.bot_name,
            ai_status_note
        );

        Ok(BotResponse::PublicMessage(help_text))
    }

    /// Stats command
    fn cmd_stats(&self, command: &BotCommand) -> Result<BotResponse> {
        let username = if command.args.is_empty() {
            &command.requester
        } else {
            &command.args[0]
        };

        let stats = self.user_stats.lock().unwrap();
        if let Some(user_stats) = stats.get(username) {
            let total_hours = user_stats.total_time_online.as_secs() / 3600;
            let avg_messages_per_day = if user_stats.session_starts > 0 {
                user_stats.total_messages / user_stats.session_starts.max(1)
            } else {
                0
            };

            let top_words: Vec<_> = user_stats
                .favorite_words
                .iter()
                .filter(|(word, _)| word.len() > 3) // Filter short words
                .collect();
            let mut top_words = top_words;
            top_words.sort_by(|a, b| b.1.cmp(a.1));
            let top_3_words: Vec<String> = top_words
                .iter()
                .take(3)
                .map(|(word, count)| format!("{} ({})", word, count))
                .collect();

            let response = format!(
                "📊 **Stats for {}:**\n\
                • Messages: {} (avg {}/session)\n\
                • Time Online: {} hours\n\
                • Sessions: {}\n\
                • First Seen: {}\n\
                • Last Seen: {}\n\
                • Kicks: {} received, {} given\n\
                • Bans: {} received, {} given\n\
                • Top Words: {}",
                username,
                user_stats.total_messages,
                avg_messages_per_day,
                total_hours,
                user_stats.session_starts,
                user_stats.first_seen.format("%Y-%m-%d %H:%M UTC"),
                user_stats.last_seen.format("%Y-%m-%d %H:%M UTC"),
                user_stats.kicks_received,
                user_stats.kicks_given,
                user_stats.bans_received,
                user_stats.bans_given,
                if top_3_words.is_empty() {
                    "None".to_string()
                } else {
                    top_3_words.join(", ")
                }
            );

            Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: response,
            })
        } else {
            Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!("❌ No statistics found for user '{}'", username),
            })
        }
    }

    /// Recall command - find message by timestamp
    fn cmd_recall(&self, command: &BotCommand) -> Result<BotResponse> {
        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Usage: @{} recall <timestamp> (format: YYYY-MM-DD HH:MM or 'HH:MM' for today)".to_string(),
            });
        }

        let timestamp_str = command.args.join(" ");
        let target_time = self.parse_timestamp(&timestamp_str)?;

        let history = self.message_history.lock().unwrap();
        let mut closest_messages: Vec<_> = history
            .iter()
            .filter(|msg| !msg.is_deleted)
            .map(|msg| {
                let diff = if msg.timestamp > target_time {
                    msg.timestamp.signed_duration_since(target_time)
                } else {
                    target_time.signed_duration_since(msg.timestamp)
                };
                (msg, diff.num_seconds().abs())
            })
            .collect();

        closest_messages.sort_by_key(|(_, diff)| *diff);

        if let Some((message, diff_seconds)) = closest_messages.first() {
            let response = format!(
                "🔍 **Closest message to {}:**\n\
                **[{}]** {}: {}\n\
                *(Time difference: {} seconds)*",
                timestamp_str,
                message.timestamp.format("%H:%M:%S"),
                message.username,
                message.content,
                diff_seconds
            );

            Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: response,
            })
        } else {
            Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ No messages found in history".to_string(),
            })
        }
    }

    /// Search command
    fn cmd_search(&self, command: &BotCommand) -> Result<BotResponse> {
        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Usage: @{} search <search term>".to_string(),
            });
        }

        let search_term = command.args.join(" ").to_lowercase();
        let history = self.message_history.lock().unwrap();

        let matches: Vec<_> = history
            .iter()
            .rev() // Most recent first
            .filter(|msg| !msg.is_deleted && msg.content.to_lowercase().contains(&search_term))
            .take(5) // Limit to 5 results
            .collect();

        if matches.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!("❌ No messages found containing '{}'", search_term),
            });
        }

        let mut response = format!("🔍 **Search results for '{}':**\n", search_term);
        for (i, message) in matches.iter().enumerate() {
            response.push_str(&format!(
                "{}. **[{}]** {}: {}\n",
                i + 1,
                message.timestamp.format("%m-%d %H:%M"),
                message.username,
                if message.content.len() > 100 {
                    format!("{}...", &message.content[..100])
                } else {
                    message.content.clone()
                }
            ));
        }

        Ok(BotResponse::PrivateMessage {
            to: command.requester.clone(),
            content: response,
        })
    }

    /// Export command
    fn cmd_export(&self, command: &BotCommand) -> Result<BotResponse> {
        let (username_filter, days_back) = if command.args.len() >= 2 {
            (
                Some(command.args[0].clone()),
                command.args[1].parse::<i64>().unwrap_or(1),
            )
        } else if command.args.len() == 1 {
            if let Ok(days) = command.args[0].parse::<i64>() {
                (None, days)
            } else {
                (Some(command.args[0].clone()), 1)
            }
        } else {
            (None, 1)
        };

        let cutoff_time = Utc::now() - chrono::Duration::days(days_back);
        let history = self.message_history.lock().unwrap();

        let messages: Vec<_> = history
            .iter()
            .filter(|msg| {
                msg.timestamp >= cutoff_time
                    && !msg.is_deleted
                    && username_filter
                        .as_ref()
                        .is_none_or(|filter| &msg.username == filter)
            })
            .take(self.config.max_export_lines)
            .collect();

        if messages.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ No messages found for export criteria".to_string(),
            });
        }

        // Generate filename
        let filename = format!(
            "chat_export_{}_{}.txt",
            username_filter.as_deref().unwrap_or("all"),
            Utc::now().format("%Y%m%d_%H%M%S")
        );

        let filepath = self.config.data_directory.join(&filename);
        let mut file = File::create(&filepath)?;

        writeln!(
            file,
            "Chat Export Generated: {}",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )?;
        writeln!(
            file,
            "Filter: {}",
            username_filter.as_deref().unwrap_or("All users")
        )?;
        writeln!(file, "Time Range: {} days back", days_back)?;
        writeln!(file, "Total Messages: {}\n", messages.len())?;
        writeln!(file, "{:-<80}", "")?;

        for message in &messages {
            writeln!(
                file,
                "[{}] {}: {}",
                message.timestamp.format("%Y-%m-%d %H:%M:%S"),
                message.username,
                message.content
            )?;
        }

        let response = format!(
            "✅ **Export completed!**\n\
            • File: {}\n\
            • Messages: {}\n\
            • Time Range: {} days\n\
            • Filter: {}",
            filename,
            messages.len(),
            days_back,
            username_filter.as_deref().unwrap_or("All users")
        );

        Ok(BotResponse::PrivateMessage {
            to: command.requester.clone(),
            content: response,
        })
    }

    /// Restore command
    fn cmd_restore(&self, command: &BotCommand) -> Result<BotResponse> {
        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Usage: @{} restore <message_id>".to_string(),
            });
        }

        let message_id: u64 = command.args[0]
            .parse()
            .map_err(|_| anyhow!("Invalid message ID"))?;

        let mut history = self.message_history.lock().unwrap();
        if let Some(message) = history.iter_mut().find(|m| m.id == Some(message_id)) {
            if message.is_deleted {
                let restored_content = message.content.clone();
                let original_author = message.username.clone();
                let original_time = message.timestamp;

                // Mark as not deleted
                message.is_deleted = false;
                message.deleted_at = None;

                // Send the restored message back to chat
                let restore_msg = format!(
                    "🔄 **Message Restored by {}:**\n[{}] {}: {}",
                    command.requester,
                    original_time.format("%H:%M:%S"),
                    original_author,
                    restored_content
                );

                return Ok(BotResponse::PublicMessage(restore_msg));
            } else {
                return Ok(BotResponse::PrivateMessage {
                    to: command.requester.clone(),
                    content: format!("❌ Message {} was not deleted", message_id),
                });
            }
        }

        Ok(BotResponse::PrivateMessage {
            to: command.requester.clone(),
            content: format!("❌ Message {} not found in history", message_id),
        })
    }

    /// Users command
    fn cmd_users(&self, command: &BotCommand) -> Result<BotResponse> {
        let users = self.current_users.lock().unwrap();
        let mut response = "👥 **Current Online Users:**\n\n".to_string();

        if !users.admin.is_empty() {
            response.push_str("**Admins:** ");
            let admin_names: Vec<String> =
                users.admin.iter().map(|(_, name)| name.clone()).collect();
            response.push_str(&admin_names.join(", "));
            response.push('\n');
        }

        if !users.staff.is_empty() {
            response.push_str("**Staff:** ");
            let staff_names: Vec<String> =
                users.staff.iter().map(|(_, name)| name.clone()).collect();
            response.push_str(&staff_names.join(", "));
            response.push('\n');
        }

        if !users.members.is_empty() {
            response.push_str("**Members:** ");
            let member_names: Vec<String> =
                users.members.iter().map(|(_, name)| name.clone()).collect();
            response.push_str(&member_names.join(", "));
            response.push('\n');
        }

        if !users.guests.is_empty() {
            response.push_str("**Guests:** ");
            let guest_names: Vec<String> =
                users.guests.iter().map(|(_, name)| name.clone()).collect();
            response.push_str(&guest_names.join(", "));
            response.push('\n');
        }

        let total_users =
            users.admin.len() + users.staff.len() + users.members.len() + users.guests.len();
        response.push_str(&format!("\n**Total:** {} users online", total_users));

        Ok(BotResponse::PrivateMessage {
            to: command.requester.clone(),
            content: response,
        })
    }

    /// Top command
    fn cmd_top(&self, command: &BotCommand) -> Result<BotResponse> {
        let category = command
            .args
            .first()
            .map(|s| s.as_str())
            .unwrap_or("messages");
        let stats = self.user_stats.lock().unwrap();

        let mut users: Vec<_> = stats.values().collect();

        match category {
            "messages" | "msgs" => {
                users.sort_by(|a, b| b.total_messages.cmp(&a.total_messages));
                let response = self.format_top_list("Most Active (Messages)", &users, |u| {
                    u.total_messages.to_string()
                });
                Ok(BotResponse::PrivateMessage {
                    to: command.requester.clone(),
                    content: response,
                })
            }
            "time" | "online" => {
                users.sort_by(|a, b| b.total_time_online.cmp(&a.total_time_online));
                let response = self.format_top_list("Most Time Online", &users, |u| {
                    format!("{:.1}h", u.total_time_online.as_secs() as f64 / 3600.0)
                });
                Ok(BotResponse::PrivateMessage {
                    to: command.requester.clone(),
                    content: response,
                })
            }
            "kicks" => {
                users.sort_by(|a, b| b.kicks_given.cmp(&a.kicks_given));
                let response =
                    self.format_top_list("Most Kicks Given", &users, |u| u.kicks_given.to_string());
                Ok(BotResponse::PrivateMessage {
                    to: command.requester.clone(),
                    content: response,
                })
            }
            _ => Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Usage: @{} top [messages|time|kicks]".to_string(),
            }),
        }
    }

    /// History command
    fn cmd_history(&self, command: &BotCommand) -> Result<BotResponse> {
        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Usage: @{} history <username> [count]".to_string(),
            });
        }

        let username = &command.args[0];
        let count: usize = command
            .args
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let history = self.message_history.lock().unwrap();
        let user_messages: Vec<_> = history
            .iter()
            .rev()
            .filter(|msg| &msg.username == username && !msg.is_deleted)
            .take(count)
            .collect();

        if user_messages.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!("❌ No message history found for '{}'", username),
            });
        }

        let mut response = format!(
            "📜 **Recent messages from {} (last {}):**\n",
            username,
            user_messages.len()
        );
        for (i, message) in user_messages.iter().enumerate() {
            response.push_str(&format!(
                "{}. **[{}]** {}\n",
                i + 1,
                message.timestamp.format("%m-%d %H:%M"),
                if message.content.len() > 80 {
                    format!("{}...", &message.content[..80])
                } else {
                    message.content.clone()
                }
            ));
        }

        Ok(BotResponse::PrivateMessage {
            to: command.requester.clone(),
            content: response,
        })
    }

    /// Summary command
    fn cmd_summary(&self, command: &BotCommand) -> Result<BotResponse> {
        let hours_back: i64 = command
            .args
            .first()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);

        let cutoff_time = Utc::now() - chrono::Duration::hours(hours_back);
        let history = self.message_history.lock().unwrap();

        let recent_messages: Vec<_> = history
            .iter()
            .filter(|msg| msg.timestamp >= cutoff_time && !msg.is_deleted)
            .collect();

        if recent_messages.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!("❌ No messages found in the last {} hours", hours_back),
            });
        }

        // Analyze activity
        let total_messages = recent_messages.len();
        let unique_users: std::collections::HashSet<_> =
            recent_messages.iter().map(|msg| &msg.username).collect();
        let user_count = unique_users.len();

        // Most active user
        let mut user_message_counts: HashMap<&String, usize> = HashMap::new();
        for message in &recent_messages {
            *user_message_counts.entry(&message.username).or_insert(0) += 1;
        }

        let most_active = user_message_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(user, count)| format!("{} ({})", user, count))
            .unwrap_or_else(|| "None".to_string());

        // Activity by hour
        let mut hourly_activity: [usize; 24] = [0; 24];
        for message in &recent_messages {
            let hour = message.timestamp.hour() as usize;
            hourly_activity[hour] += 1;
        }

        let peak_hour = hourly_activity
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .map(|(hour, count)| format!("{}:00 ({} msgs)", hour, count))
            .unwrap_or_else(|| "None".to_string());

        let response = format!(
            "📊 **Chat Summary (last {} hours):**\n\
            • Total Messages: {}\n\
            • Active Users: {}\n\
            • Most Active User: {}\n\
            • Peak Hour: {}\n\
            • Messages per Hour: {:.1}",
            hours_back,
            total_messages,
            user_count,
            most_active,
            peak_hour,
            total_messages as f64 / hours_back as f64
        );

        Ok(BotResponse::PrivateMessage {
            to: command.requester.clone(),
            content: response,
        })
    }

    /// Status command
    fn cmd_status(&self, _command: &BotCommand) -> Result<BotResponse> {
        let history_count = self.message_history.lock().unwrap().len();
        let user_count = self.user_stats.lock().unwrap().len();
        let uptime = SystemTime::now()
            .duration_since(*self.last_save.lock().unwrap())
            .unwrap_or_default();

        let is_running = *self.running.lock().unwrap();

        // Get AI configuration status (no actual API calls)
        let ai_status = if self.ai_service.is_some() {
            "✅ Configured"
        } else {
            "❌ Disabled"
        };

        let response = format!(
            "🤖 **{} Status:**\n\
            • Status: {}\n\
            • Messages Tracked: {}\n\
            • Users Tracked: {}\n\
            • Last Save: {:.1} minutes ago\n\
            • Data Directory: {}\n\
            • AI Integration: {}\n\
            • Max History: {}\n\
            \n\
            ℹ️  **Available Commands:**\n\
            Basic commands (always work): help, stats, recall, export, search, history, top, users, restore, status\n\
            Admin commands: purge\n\
            AI commands: Available via ChatOps when AI is functional",
            self.config.bot_name,
            if is_running {
                "🟢 Online"
            } else {
                "🔴 Offline"
            },
            history_count,
            user_count,
            uptime.as_secs() as f64 / 60.0,
            self.config.data_directory.display(),
            ai_status,
            self.config.max_message_history
        );

        Ok(BotResponse::PublicMessage(response))
    }

    /// Purge command (admin only)
    fn cmd_purge(&self, command: &BotCommand) -> Result<BotResponse> {
        if !self.is_admin(&command.requester) {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Admin access required for this command".to_string(),
            });
        }

        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Usage: @{} purge <username>".to_string(),
            });
        }

        let username = &command.args[0];

        // Remove from user stats
        let mut stats = self.user_stats.lock().unwrap();
        if stats.remove(username).is_some() {
            drop(stats);

            // Remove from message history
            let mut history = self.message_history.lock().unwrap();
            history.retain(|msg| msg.username != *username);

            let response = format!(
                "✅ **Purged all data for user '{}'**\n\
                • Removed user statistics\n\
                • Removed message history\n\
                • Action performed by: {}",
                username, command.requester
            );

            // Save data after purge
            if let Err(e) = self.save_data() {
                warn!("Failed to save data after purge: {}", e);
            }

            Ok(BotResponse::PublicMessage(response))
        } else {
            Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!("❌ No data found for user '{}'", username),
            })
        }
    }

    /// Helper function to format top lists
    fn format_top_list<F>(&self, title: &str, users: &[&UserStats], value_fn: F) -> String
    where
        F: Fn(&UserStats) -> String,
    {
        let mut response = format!("🏆 **{}:**\n", title);

        for (i, user) in users.iter().take(10).enumerate() {
            let value = value_fn(user);
            response.push_str(&format!("{}. {} - {}\n", i + 1, user.username, value));
        }

        if users.is_empty() {
            response.push_str("No data available yet.");
        }

        response
    }

    /// Parse timestamp string
    fn parse_timestamp(&self, timestamp_str: &str) -> Result<DateTime<Utc>> {
        let now = Utc::now();

        // Try parsing as HH:MM for today
        if let Ok(naive_time) = chrono::NaiveTime::parse_from_str(timestamp_str, "%H:%M") {
            let today = now.date_naive();
            let naive_datetime = today.and_time(naive_time);
            return Ok(naive_datetime.and_utc());
        }

        // Try parsing as YYYY-MM-DD HH:MM
        if let Ok(naive_datetime) =
            chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M")
        {
            return Ok(naive_datetime.and_utc());
        }

        // Try parsing as MM-DD HH:MM (current year)
        if let Ok(naive_datetime) = chrono::NaiveDateTime::parse_from_str(
            &format!("{}-{}", now.year(), timestamp_str),
            "%Y-%m-%d %H:%M",
        ) {
            return Ok(naive_datetime.and_utc());
        }

        Err(anyhow!(
            "Invalid timestamp format. Use 'HH:MM', 'MM-DD HH:MM', or 'YYYY-MM-DD HH:MM'"
        ))
    }

    /// Check if user is admin
    fn is_admin(&self, username: &str) -> bool {
        self.config.admin_users.contains(&username.to_string())
    }

    /// Kick command (members only)
    fn cmd_kick(&self, command: &BotCommand) -> Result<BotResponse> {
        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!(
                    "❌ Usage: @{} kick <username> [reason]",
                    self.config.bot_name
                ),
            });
        }

        let username = &command.args[0];
        let reason = if command.args.len() > 1 {
            command.args[1..].join(" ")
        } else {
            "No reason provided".to_string()
        };

        // Protect Dasho from being kicked
        if username.to_lowercase() == "dasho" {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Cannot kick Dasho - protected user".to_string(),
            });
        }

        Ok(BotResponse::Action(BotAction::Kick {
            username: username.clone(),
            reason,
        }))
    }

    /// Ban command (members only)
    fn cmd_ban(&self, command: &BotCommand) -> Result<BotResponse> {
        if command.args.is_empty() {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: format!(
                    "❌ Usage: @{} ban <username> [reason]",
                    self.config.bot_name
                ),
            });
        }

        let username = &command.args[0];
        let reason = if command.args.len() > 1 {
            command.args[1..].join(" ")
        } else {
            "No reason provided".to_string()
        };

        // Protect Dasho from being banned
        if username.to_lowercase().contains("dasho") {
            return Ok(BotResponse::PrivateMessage {
                to: command.requester.clone(),
                content: "❌ Cannot ban Dasho - protected user".to_string(),
            });
        }

        Ok(BotResponse::Action(BotAction::Ban {
            username: username.clone(),
            reason,
        }))
    }

    /// Send bot response with context-aware channel selection
    fn send_response_with_context(
        &self,
        response: BotResponse,
        command: &BotCommand,
    ) -> Result<()> {
        match response {
            BotResponse::PublicMessage(content) => {
                // Reply in the same channel as the command came from
                let target = match command.channel_context.as_deref() {
                    Some("members") => {
                        log::info!("Bot '{}' responding in [M] channel", self.config.bot_name);
                        Some(crate::SEND_TO_MEMBERS.to_string())
                    },
                    Some("staff") => {
                        log::info!("Bot '{}' responding in [S] channel", self.config.bot_name);
                        Some(crate::SEND_TO_STAFFS.to_string())
                    },
                    Some("admin") => {
                        log::info!("Bot '{}' responding in [A] channel", self.config.bot_name);
                        Some(crate::SEND_TO_ADMINS.to_string())
                    },
                    _ => {
                        log::info!("Bot '{}' responding in main chat (context: {:?})", 
                            self.config.bot_name, command.channel_context);
                        None // Main chat (public channel)
                    }
                };

                if let Err(e) = self.tx.try_send(PostType::Post(content, target)) {
                    warn!(
                        "Bot '{}' failed to send public message: {}",
                        self.config.bot_name, e
                    );
                }
            }
            BotResponse::PrivateMessage { to, content } => {
                if let Err(e) = self.tx.try_send(PostType::PM(to, content)) {
                    warn!(
                        "Bot '{}' failed to send private message: {}",
                        self.config.bot_name, e
                    );
                }
            }
            BotResponse::Action(action) => {
                match action {
                    BotAction::Kick { username, reason } => {
                        if let Err(e) = self.tx.try_send(PostType::Kick(reason, username)) {
                            warn!(
                                "Failed to send kick action (channel may be disconnected): {}",
                                e
                            );
                        }
                    }
                    BotAction::Ban {
                        username,
                        reason: _,
                    } => {
                        // Note: Implement ban action based on BHCLI's ban system
                        let ban_msg = format!("/ban {}", username);
                        if let Err(e) = self
                            .tx
                            .try_send(PostType::Post(ban_msg, Some("0".to_string())))
                        {
                            warn!(
                                "Failed to send ban command (channel may be disconnected): {}",
                                e
                            );
                        }
                    }
                    BotAction::Warn { username, message } => {
                        let warn_msg = format!("⚠️ @{}: {}", username, message);
                        if let Err(e) = self
                            .tx
                            .try_send(PostType::Post(warn_msg, Some("0".to_string())))
                        {
                            warn!(
                                "Failed to send warning message (channel may be disconnected): {}",
                                e
                            );
                        }
                    }
                    BotAction::SaveChatLog { filename: _ } => {
                        // Handled by export command
                    }
                    BotAction::RestoreMessage { message_id: _ } => {
                        // Handled by restore command
                    }
                }
            }
            BotResponse::Error(error) => {
                error!("Bot error: {}", error);
            }
        }
        Ok(())
    }

    /// Update user statistics
    fn update_user_stats(
        &self,
        username: &str,
        content: &str,
        message_type: &MessageType,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let mut stats = self.user_stats.lock().unwrap();
        let user_stats = stats.entry(username.to_string()).or_default();

        // Update basic stats
        if user_stats.username.is_empty() {
            user_stats.username = username.to_string();
            user_stats.first_seen = timestamp;
        }
        user_stats.last_seen = timestamp;
        user_stats.total_messages += 1;

        // Update hourly activity
        let hour = timestamp.hour() as usize;
        if hour < 24 {
            user_stats.hourly_activity[hour] += 1;
        }

        // Update daily activity
        let date_key = timestamp.format("%Y-%m-%d").to_string();
        *user_stats.daily_activity.entry(date_key).or_insert(0) += 1;

        // Update word frequency
        let words: Vec<&str> = content.split_whitespace().collect();
        for word in words {
            let clean_word = word
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>();

            if clean_word.len() > 3 {
                *user_stats.favorite_words.entry(clean_word).or_insert(0) += 1;
            }
        }

        // Update message type specific stats
        match message_type {
            MessageType::Kick { by, .. } => {
                if by == username {
                    user_stats.kicks_given += 1;
                } else {
                    user_stats.kicks_received += 1;
                }
            }
            MessageType::Ban { by, .. } => {
                if by == username {
                    user_stats.bans_given += 1;
                } else {
                    user_stats.bans_received += 1;
                }
            }
            MessageType::Join => {
                user_stats.session_starts += 1;
            }
            _ => {}
        }

        Ok(())
    }

    /// Start auto-save thread
    fn start_auto_save_thread(&self) {
        let message_history = Arc::clone(&self.message_history);
        let user_stats = Arc::clone(&self.user_stats);
        let running = Arc::clone(&self.running);
        let last_save = Arc::clone(&self.last_save);
        let data_dir = self.config.data_directory.clone();
        let save_interval = self.config.auto_save_interval;

        thread::spawn(move || {
            while *running.lock().unwrap() {
                thread::sleep(save_interval);

                if let Err(e) = Self::save_data_to_disk(&message_history, &user_stats, &data_dir) {
                    error!("Auto-save failed: {}", e);
                } else {
                    *last_save.lock().unwrap() = SystemTime::now();
                }
            }
        });
    }

    /// Save data to disk
    fn save_data_to_disk(
        message_history: &Arc<Mutex<Vec<BotChatMessage>>>,
        user_stats: &Arc<Mutex<HashMap<String, UserStats>>>,
        data_dir: &Path,
    ) -> Result<()> {
        // Save message history
        let history_file = data_dir.join("message_history.json");
        let history = message_history.lock().unwrap();
        let history_json = serde_json::to_string_pretty(&*history)?;
        std::fs::write(history_file, history_json)?;

        // Save user stats
        let stats_file = data_dir.join("user_stats.json");
        let stats = user_stats.lock().unwrap();
        let stats_json = serde_json::to_string_pretty(&*stats)?;
        std::fs::write(stats_file, stats_json)?;

        Ok(())
    }

    /// Save all data
    pub fn save_data(&self) -> Result<()> {
        Self::save_data_to_disk(
            &self.message_history,
            &self.user_stats,
            &self.config.data_directory,
        )
    }

    /// Load existing data
    fn load_data(&self) -> Result<()> {
        // Load message history
        let history_file = self.config.data_directory.join("message_history.json");
        if history_file.exists() {
            let history_json = std::fs::read_to_string(history_file)?;
            if let Ok(history) = serde_json::from_str::<Vec<BotChatMessage>>(&history_json) {
                *self.message_history.lock().unwrap() = history;
                info!(
                    "Loaded {} messages from history",
                    self.message_history.lock().unwrap().len()
                );
            }
        }

        // Load user stats
        let stats_file = self.config.data_directory.join("user_stats.json");
        if stats_file.exists() {
            let stats_json = std::fs::read_to_string(stats_file)?;
            if let Ok(stats) = serde_json::from_str::<HashMap<String, UserStats>>(&stats_json) {
                *self.user_stats.lock().unwrap() = stats;
                info!(
                    "Loaded {} user statistics",
                    self.user_stats.lock().unwrap().len()
                );
            }
        }

        Ok(())
    }

    /// Check if bot is running
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}
