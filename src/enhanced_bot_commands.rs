use crate::enhanced_bot_system::{
    EnhancedBotSystem, EnhancedBotResponse, BotChannel, EmbeddedContent, 
    EmbeddedField, ModerationAction, CustomCommand
};
use crate::{Users};
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use std::collections::HashMap;

impl EnhancedBotSystem {
    /// Enhanced help command with categorized features
    pub fn cmd_help(&self, username: &str, channel: &BotChannel) -> Result<EnhancedBotResponse> {
        let is_admin = self.is_admin(username);
        let content = if is_admin {
            self.generate_admin_help()
        } else {
            self.generate_user_help()
        };
        
        Ok(EnhancedBotResponse::EmbeddedMessage {
            content: EmbeddedContent {
                title: Some(format!("🤖 {} Commands", self.config.bot_name)),
                description: "Available commands and features".to_string(),
                color: Some("#00FF00".to_string()),
                fields: content,
                footer: Some("Use @bot <command> for more details".to_string()),
                thumbnail: None,
            },
            channel: BotChannel::Current,
        })
    }
    
    /// Enhanced statistics with moderation data
    pub fn cmd_enhanced_stats(
        &self, 
        username: &str, 
        args: &[&str], 
        channel: &BotChannel
    ) -> Result<EnhancedBotResponse> {
        let target_user = args.get(0).unwrap_or(&username);
        let user_stats = self.user_stats.lock().unwrap();
        
        if let Some(stats) = user_stats.get(*target_user) {
            let fields = vec![
                EmbeddedField {
                    name: "📊 Activity".to_string(),
                    value: format!(
                        "Messages: {}\nLevel: {} ({}xp)\nTime Online: {} hrs",
                        stats.total_messages,
                        stats.level,
                        stats.experience_points,
                        stats.total_time_online.num_hours()
                    ),
                    inline: true,
                },
                EmbeddedField {
                    name: "🛡️ Moderation".to_string(),
                    value: format!(
                        "Warnings: {}\nKicks: {}\nReputation: {}",
                        stats.warnings_received,
                        stats.kicks_received,
                        stats.reputation_score
                    ),
                    inline: true,
                },
                EmbeddedField {
                    name: "🏆 Achievements".to_string(),
                    value: format!("{} unlocked", stats.achievements.len()),
                    inline: true,
                },
            ];
            
            Ok(EnhancedBotResponse::EmbeddedMessage {
                content: EmbeddedContent {
                    title: Some(format!("📊 Stats for {}", target_user)),
                    description: format!("Member since: {}", stats.first_seen.format("%Y-%m-%d")),
                    color: Some("#0099FF".to_string()),
                    fields,
                    footer: Some(format!("Last seen: {}", stats.last_seen.format("%Y-%m-%d %H:%M"))),
                    thumbnail: None,
                },
                channel: BotChannel::Current,
            })
        } else {
            Ok(EnhancedBotResponse::ChannelMessage {
                content: format!("❌ No stats found for user: {}", target_user),
                channel: BotChannel::Current,
            })
        }
    }
    
    /// Comprehensive moderation command
    pub fn cmd_moderation(
        &self,
        username: &str,
        args: &[&str],
        channel: &BotChannel,
        users: &Users,
    ) -> Result<EnhancedBotResponse> {
        if !self.is_moderator(username, users) {
            return Ok(EnhancedBotResponse::PrivateMessage {
                to: username.to_string(),
                content: "❌ Insufficient permissions for moderation commands".to_string(),
            });
        }
        
        if args.is_empty() {
            return self.show_moderation_help(channel);
        }
        
        match args[0] {
            "warn" => self.cmd_mod_warn(username, &args[1..], channel),
            "kick" => self.cmd_mod_kick(username, &args[1..], channel),
            "ban" => self.cmd_mod_ban(username, &args[1..], channel),
            "mute" => self.cmd_mod_mute(username, &args[1..], channel),
            "cleanup" => self.cmd_mod_cleanup(username, &args[1..], channel),
            "config" => self.cmd_mod_config(username, &args[1..], channel),
            "stats" => self.cmd_mod_stats(username, &args[1..], channel),
            _ => Ok(EnhancedBotResponse::ChannelMessage {
                content: format!("❌ Unknown moderation command: {}", args[0]),
                channel: BotChannel::Current,
            }),
        }
    }
    
    /// Role management system
    pub fn cmd_roles(
        &self,
        username: &str,
        args: &[&str],
        channel: &BotChannel,
    ) -> Result<EnhancedBotResponse> {
        let role_manager = self.role_manager.lock().unwrap();
        
        if args.is_empty() {
            // Show user's current roles
            if let Some(roles) = role_manager.user_roles.get(username) {
                let role_list = if roles.is_empty() {
                    "No special roles".to_string()
                } else {
                    roles.join(", ")
                };
                
                Ok(EnhancedBotResponse::ChannelMessage {
                    content: format!("🎭 **Roles for {}:** {}", username, role_list),
                    channel: BotChannel::Current,
                })
            } else {
                Ok(EnhancedBotResponse::ChannelMessage {
                    content: format!("🎭 **{}** has no special roles", username),
                    channel: BotChannel::Current,
                })
            }
        } else {
            match args[0] {
                "list" => self.cmd_roles_list(channel),
                "assign" => self.cmd_roles_assign(username, &args[1..], channel),
                "remove" => self.cmd_roles_remove(username, &args[1..], channel),
                "info" => self.cmd_roles_info(&args[1..], channel),
                _ => Ok(EnhancedBotResponse::ChannelMessage {
                    content: "❌ Unknown role command. Use: list, assign, remove, info".to_string(),
                    channel: BotChannel::Current,
                }),
            }
        }
    }
    
    /// Warning system management
    pub fn cmd_warnings(
        &self,
        username: &str,
        args: &[&str],
        channel: &BotChannel,
    ) -> Result<EnhancedBotResponse> {
        let target_user = args.get(0).unwrap_or(&username);
        let moderation = self.moderation_engine.lock().unwrap();
        
        if let Some(warning_system) = moderation.warning_system.get(*target_user) {
            let active_warnings: Vec<_> = warning_system.warnings.iter()
                .filter(|w| w.expires_at > Utc::now())
                .collect();
            
            if active_warnings.is_empty() {
                Ok(EnhancedBotResponse::ChannelMessage {
                    content: format!("✅ {} has no active warnings", target_user),
                    channel: BotChannel::Current,
                })
            } else {
                let warning_list = active_warnings.iter()
                    .enumerate()
                    .map(|(i, w)| format!(
                        "{}. **{}** ({}pts) - Expires: {}",
                        i + 1,
                        w.reason,
                        w.points,
                        w.expires_at.format("%m/%d %H:%M")
                    ))
                    .collect::<Vec<_>>()
                    .join("\n");
                
                Ok(EnhancedBotResponse::EmbeddedMessage {
                    content: EmbeddedContent {
                        title: Some(format!("⚠️ Active Warnings for {}", target_user)),
                        description: format!("Total Points: {}", warning_system.total_points),
                        color: Some("#FF6600".to_string()),
                        fields: vec![EmbeddedField {
                            name: "Warnings".to_string(),
                            value: warning_list,
                            inline: false,
                        }],
                        footer: None,
                        thumbnail: None,
                    },
                    channel: BotChannel::Current,
                })
            }
        } else {
            Ok(EnhancedBotResponse::ChannelMessage {
                content: format!("✅ {} has no warning history", target_user),
                channel: BotChannel::Current,
            })
        }
    }
    
    /// Leaderboard with multiple categories
    pub fn cmd_leaderboard(
        &self,
        username: &str,
        args: &[&str],
        channel: &BotChannel,
    ) -> Result<EnhancedBotResponse> {
        let category = args.get(0).unwrap_or(&"level");
        let user_stats = self.user_stats.lock().unwrap();
        
        let mut users: Vec<_> = user_stats.values().collect();
        
        match *category {
            "messages" => users.sort_by(|a, b| b.total_messages.cmp(&a.total_messages)),
            "level" => users.sort_by(|a, b| b.level.cmp(&a.level)),
            "xp" => users.sort_by(|a, b| b.experience_points.cmp(&a.experience_points)),
            "reputation" => users.sort_by(|a, b| b.reputation_score.cmp(&a.reputation_score)),
            _ => {
                return Ok(EnhancedBotResponse::ChannelMessage {
                    content: "❌ Invalid category. Use: messages, level, xp, reputation".to_string(),
                    channel: BotChannel::Current,
                });
            }
        }
        
        let top_users = users.iter().take(10)
            .enumerate()
            .map(|(i, stats)| {
                let value = match *category {
                    "messages" => stats.total_messages.to_string(),
                    "level" => format!("{} ({}xp)", stats.level, stats.experience_points),
                    "xp" => stats.experience_points.to_string(),
                    "reputation" => stats.reputation_score.to_string(),
                    _ => "N/A".to_string(),
                };
                format!("{}. **{}**: {}", i + 1, stats.username, value)
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(EnhancedBotResponse::EmbeddedMessage {
            content: EmbeddedContent {
                title: Some(format!("🏆 Leaderboard - {}", category.to_uppercase())),
                description: "Top 10 users".to_string(),
                color: Some("#FFD700".to_string()),
                fields: vec![EmbeddedField {
                    name: "Rankings".to_string(),
                    value: if top_users.is_empty() { "No data available".to_string() } else { top_users },
                    inline: false,
                }],
                footer: Some(format!("Requested by {}", username)),
                thumbnail: None,
            },
            channel: BotChannel::Current,
        })
    }
    
    /// Level and experience system
    pub fn cmd_level(
        &self,
        username: &str,
        args: &[&str],
        channel: &BotChannel,
    ) -> Result<EnhancedBotResponse> {
        let target_user = args.get(0).unwrap_or(&username);
        let user_stats = self.user_stats.lock().unwrap();
        
        if let Some(stats) = user_stats.get(*target_user) {
            let automation = self.automation_engine.lock().unwrap();
            let next_level_xp = self.calculate_next_level_xp(stats.level, &automation.activity_tracker.level_formula);
            let xp_needed = next_level_xp.saturating_sub(stats.experience_points);
            
            let progress_bar = self.generate_progress_bar(
                stats.experience_points,
                next_level_xp,
                20
            );
            
            Ok(EnhancedBotResponse::EmbeddedMessage {
                content: EmbeddedContent {
                    title: Some(format!("🎯 Level Info for {}", target_user)),
                    description: format!("Level {} • {} XP", stats.level, stats.experience_points),
                    color: Some("#9966FF".to_string()),
                    fields: vec![
                        EmbeddedField {
                            name: "Progress".to_string(),
                            value: format!("{}\n{} XP needed for level {}", 
                                progress_bar, xp_needed, stats.level + 1),
                            inline: false,
                        },
                        EmbeddedField {
                            name: "Recent Achievements".to_string(),
                            value: if stats.achievements.is_empty() {
                                "None yet".to_string()
                            } else {
                                stats.achievements.iter()
                                    .rev()
                                    .take(3)
                                    .map(|a| format!("🏅 {}", a.name))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            },
                            inline: true,
                        },
                    ],
                    footer: Some("Earn XP by chatting and participating!".to_string()),
                    thumbnail: None,
                },
                channel: BotChannel::Current,
            })
        } else {
            Ok(EnhancedBotResponse::ChannelMessage {
                content: format!("❌ No level data found for {}", target_user),
                channel: BotChannel::Current,
            })
        }
    }
    
    /// Enhanced status with system information
    pub fn cmd_enhanced_status(&self, channel: &BotChannel) -> Result<EnhancedBotResponse> {
        let message_count = self.message_history.lock().unwrap().len();
        let user_count = self.user_stats.lock().unwrap().len();
        let uptime = chrono::Utc::now(); // This would be actual uptime calculation
        
        let moderation_stats = {
            let moderation = self.moderation_engine.lock().unwrap();
            format!(
                "Auto-mod: {}\nSpam filtered: {}\nWarnings issued: {}",
                if self.config.auto_moderation_enabled { "✅" } else { "❌" },
                moderation.spam_tracker.len(),
                moderation.warning_system.len()
            )
        };
        
        let automation_stats = {
            let automation = self.automation_engine.lock().unwrap();
            format!(
                "Welcome msgs: {}\nScheduled tasks: {}\nRole automation: {}",
                if automation.welcome_manager.enabled { "✅" } else { "❌" },
                automation.scheduled_tasks.len(),
                if automation.role_automation.enabled { "✅" } else { "❌" }
            )
        };
        
        Ok(EnhancedBotResponse::EmbeddedMessage {
            content: EmbeddedContent {
                title: Some(format!("🤖 {} System Status", self.config.bot_name)),
                description: "Bot health and statistics".to_string(),
                color: Some("#00FF00".to_string()),
                fields: vec![
                    EmbeddedField {
                        name: "📊 Data".to_string(),
                        value: format!("Messages tracked: {}\nUsers tracked: {}", message_count, user_count),
                        inline: true,
                    },
                    EmbeddedField {
                        name: "🛡️ Moderation".to_string(),
                        value: moderation_stats,
                        inline: true,
                    },
                    EmbeddedField {
                        name: "🤖 Automation".to_string(),
                        value: automation_stats,
                        inline: true,
                    },
                ],
                footer: Some("Enhanced Bot System v2.0".to_string()),
                thumbnail: None,
            },
            channel: BotChannel::Current,
        })
    }
    
    // Helper methods
    fn generate_admin_help(&self) -> Vec<EmbeddedField> {
        vec![
            EmbeddedField {
                name: "👥 User Commands".to_string(),
                value: "`stats`, `level`, `warnings`, `leaderboard`, `search`".to_string(),
                inline: false,
            },
            EmbeddedField {
                name: "🛡️ Moderation".to_string(),
                value: "`mod warn/kick/ban/mute`, `mod config`, `automod`".to_string(),
                inline: false,
            },
            EmbeddedField {
                name: "🎭 Role Management".to_string(),
                value: "`roles assign/remove/list`, `permissions`".to_string(),
                inline: false,
            },
            EmbeddedField {
                name: "🤖 Automation".to_string(),
                value: "`schedule`, `welcome`, `cleanup`, `config`".to_string(),
                inline: false,
            },
        ]
    }
    
    fn generate_user_help(&self) -> Vec<EmbeddedField> {
        vec![
            EmbeddedField {
                name: "📊 Statistics".to_string(),
                value: "`stats [user]`, `level [user]`, `leaderboard [type]`".to_string(),
                inline: false,
            },
            EmbeddedField {
                name: "🔍 Information".to_string(),
                value: "`users`, `status`, `search <term>`".to_string(),
                inline: false,
            },
            EmbeddedField {
                name: "🎭 Profile".to_string(),
                value: "`roles`, `achievements`, `warnings`".to_string(),
                inline: false,
            },
        ]
    }
    
    fn is_admin(&self, username: &str) -> bool {
        self.config.admins.contains(&username.to_string())
    }
    
    fn is_moderator(&self, username: &str, users: &Users) -> bool {
        self.is_admin(username) || 
        users.staff.iter().any(|(_, name)| name == username) ||
        users.admin.iter().any(|(_, name)| name == username)
    }
    
    fn is_staff(&self, username: &str, users: &Users) -> bool {
        users.staff.iter().any(|(_, name)| name == username)
    }
    
    pub fn update_user_activity(&self, username: &str, is_member: bool) -> Result<()> {
        let mut user_stats = self.user_stats.lock().unwrap();
        let stats = user_stats.entry(username.to_string())
            .or_insert_with(|| self.create_new_user_stats(username, is_member));
        
        stats.total_messages += 1;
        stats.last_seen = Utc::now();
        stats.current_session_messages += 1;
        
        // Award experience points
        let automation = self.automation_engine.lock().unwrap();
        let xp_gain = automation.activity_tracker.xp_per_message;
        stats.experience_points += xp_gain;
        
        // Check for level up
        let new_level = self.calculate_level(stats.experience_points, &automation.activity_tracker.level_formula);
        if new_level > stats.level {
            stats.level = new_level;
            // Could trigger level up achievement/notification here
        }
        
        Ok(())
    }
    
    pub fn add_to_history(
        &self,
        username: &str,
        content: &str,
        message_type: crate::bot_system::MessageType,
        message_id: Option<u64>,
        timestamp: chrono::DateTime<Utc>,
    ) -> Result<()> {
        // Convert MessageType and add to enhanced message history
        let mut history = self.message_history.lock().unwrap();
        
        // Keep history within limits
        if history.len() >= self.config.max_message_history {
            history.remove(0);
        }
        
        let bot_message = crate::bot_system::BotChatMessage {
            id: message_id,
            timestamp,
            username: username.to_string(),
            content: content.to_string(),
            message_type: crate::bot_system::MessageType::Normal, // Convert as needed
            is_deleted: false,
            deleted_at: None,
            edit_history: vec![],
        };
        
        history.push(bot_message);
        Ok(())
    }
    
    // Additional helper methods would be implemented here...
    fn create_new_user_stats(&self, username: &str, is_member: bool) -> crate::enhanced_bot_system::EnhancedUserStats {
        use crate::enhanced_bot_system::EnhancedUserStats;
        use crate::chatops::UserRole;
        
        EnhancedUserStats {
            username: username.to_string(),
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            total_messages: 0,
            total_time_online: Duration::zero(),
            session_count: 1,
            average_messages_per_session: 0.0,
            most_used_words: HashMap::new(),
            hourly_activity: HashMap::new(),
            daily_activity: HashMap::new(),
            current_session_start: Some(Utc::now()),
            current_session_messages: 0,
            warnings_received: 0,
            kicks_received: 0,
            bans_received: 0,
            warnings_given: 0,
            kicks_given: 0,
            bans_given: 0,
            reputation_score: 100, // Start with neutral reputation
            offense_history: vec![],
            last_offense: None,
            experience_points: 0,
            level: 1,
            achievements: vec![],
            current_role: if is_member { UserRole::Member } else { UserRole::Guest },
        }
    }
    
    fn calculate_level(&self, xp: u64, formula: &crate::enhanced_bot_system::LevelFormula) -> u32 {
        match formula {
            crate::enhanced_bot_system::LevelFormula::Linear(xp_per_level) => {
                (xp / xp_per_level) as u32 + 1
            }
            crate::enhanced_bot_system::LevelFormula::Exponential(base) => {
                let mut level = 1;
                let mut required_xp = 100; // Base XP for level 2
                while xp >= required_xp {
                    level += 1;
                    required_xp = (required_xp as f64 * base) as u64;
                }
                level
            }
            crate::enhanced_bot_system::LevelFormula::Custom(_) => {
                // Could implement custom formula parsing
                1
            }
        }
    }
    
    fn calculate_next_level_xp(&self, current_level: u32, formula: &crate::enhanced_bot_system::LevelFormula) -> u64 {
        match formula {
            crate::enhanced_bot_system::LevelFormula::Linear(xp_per_level) => {
                (current_level as u64 + 1) * xp_per_level
            }
            crate::enhanced_bot_system::LevelFormula::Exponential(base) => {
                let mut required_xp = 100;
                for _ in 1..current_level {
                    required_xp = (required_xp as f64 * base) as u64;
                }
                required_xp
            }
            crate::enhanced_bot_system::LevelFormula::Custom(_) => 1000,
        }
    }
    
    fn generate_progress_bar(&self, current: u64, max: u64, width: usize) -> String {
        let percentage = if max > 0 { current as f64 / max as f64 } else { 0.0 };
        let filled = (percentage * width as f64) as usize;
        let empty = width - filled;
        
        format!("[{}{}] {:.1}%",
            "█".repeat(filled),
            "░".repeat(empty),
            percentage * 100.0
        )
    }
    
    fn format_embedded_content(&self, content: &EmbeddedContent) -> String {
        let mut formatted = String::new();
        
        if let Some(ref title) = content.title {
            formatted.push_str(&format!("**{}**\n", title));
        }
        
        if !content.description.is_empty() {
            formatted.push_str(&format!("{}\n\n", content.description));
        }
        
        for field in &content.fields {
            formatted.push_str(&format!("**{}**\n{}\n\n", field.name, field.value));
        }
        
        if let Some(ref footer) = content.footer {
            formatted.push_str(&format!("*{}*", footer));
        }
        
        formatted
    }
    
    // Stub implementations for moderation commands
    fn show_moderation_help(&self, channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "🛡️ **Moderation Commands:**\n• `warn <user> <reason>`\n• `kick <user> <reason>`\n• `ban <user> <reason>`\n• `mute <user> <duration> <reason>`\n• `cleanup <count>`\n• `config <setting> <value>`".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_mod_warn(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        if args.len() < 2 {
            return Ok(EnhancedBotResponse::ChannelMessage {
                content: "❌ Usage: mod warn <user> <reason>".to_string(),
                channel: BotChannel::Current,
            });
        }
        
        let target = args[0];
        let reason = args[1..].join(" ");
        
        // Implementation for warning system would go here
        Ok(EnhancedBotResponse::ChannelMessage {
            content: format!("⚠️ {} warned {} for: {}", moderator, target, reason),
            channel: BotChannel::ModLog.into(),
        })
    }
    
    // Additional command stubs...
    fn cmd_mod_kick(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Kick command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_mod_ban(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Ban command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_mod_mute(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Mute command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_mod_cleanup(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Cleanup command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_mod_config(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Config command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_mod_stats(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Mod stats command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_roles_list(&self, channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Roles list command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_roles_assign(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Role assign command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_roles_remove(&self, moderator: &str, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Role remove command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    fn cmd_roles_info(&self, args: &[&str], channel: &BotChannel) -> Result<EnhancedBotResponse> {
        Ok(EnhancedBotResponse::ChannelMessage {
            content: "Role info command implementation".to_string(),
            channel: BotChannel::Current,
        })
    }
    
    // Additional automation methods would be implemented
    pub fn run_automation_tasks(
        &self, 
        username: &str, 
        content: &str, 
        channel: &BotChannel, 
        is_member: bool
    ) -> Result<()> {
        // Welcome messages, role automation, etc. would be implemented here
        Ok(())
    }
    
    pub fn handle_moderation_violations(
        &self,
        username: &str,
        violations: Vec<String>,
        channel: &BotChannel,
        message_id: Option<u64>,
    ) -> Result<EnhancedBotResponse> {
        // Handle moderation violations and return appropriate response
        Ok(EnhancedBotResponse::ChannelMessage {
            content: format!("Moderation action taken against {} for: {}", username, violations.join(", ")),
            channel: BotChannel::ModLog.into(),
        })
    }
    
    pub fn handle_custom_command(
        &self,
        command: &str,
        username: &str,
        channel: &BotChannel,
    ) -> Result<Option<EnhancedBotResponse>> {
        if let Some(custom_cmd) = self.config.custom_commands.get(command) {
            let mut response = custom_cmd.response.clone();
            response = response.replace("{user}", username);
            response = response.replace("{bot}", &self.config.bot_name);
            
            Ok(Some(EnhancedBotResponse::ChannelMessage {
                content: response,
                channel: BotChannel::Current,
            }))
        } else {
            Ok(None)
        }
    }
}

// Removed conflicting From implementation