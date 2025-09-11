use crate::enhanced_bot_system::{EnhancedBotSystem, EnhancedBotConfig, BotChannel, EnhancedBotResponse};
use crate::bot_system::{BotSystem, BotCommand, BotResponse, MessageType};
use crate::chatops::{ChatOpsRouter, UserRole};
use crate::ai_service::AIService;
use crate::{PostType, Users};
use anyhow::Result;
use crossbeam_channel::Sender;
use log::{error, info, warn};
use std::sync::{Arc, Mutex};

/// Integration layer between existing bot system and enhanced bot system
pub struct BotIntegration {
    pub enhanced_bot: Option<Arc<Mutex<EnhancedBotSystem>>>,
    pub legacy_bot: Arc<BotSystem>,
    pub migration_mode: bool,
}

impl BotIntegration {
    pub fn new(
        legacy_bot: Arc<BotSystem>,
        enhanced_config: Option<EnhancedBotConfig>,
        tx: Sender<PostType>,
        ai_service: Option<Arc<AIService>>,
        chatops_router: Option<Arc<ChatOpsRouter>>,
    ) -> Result<Self> {
        let enhanced_bot = if let Some(config) = enhanced_config {
            let enhanced = EnhancedBotSystem::new(config, tx, ai_service, chatops_router)?;
            Some(Arc::new(Mutex::new(enhanced)))
        } else {
            None
        };
        
        Ok(Self {
            enhanced_bot,
            legacy_bot,
            migration_mode: enhanced_bot.is_some(),
        })
    }
    
    /// Process message with proper channel detection
    pub fn process_message_enhanced(
        &self,
        username: &str,
        content: &str,
        message_type: MessageType,
        message_id: Option<u64>,
        channel_source: BotChannel, // Fixed: Pass actual channel source, not content-based detection
        is_member: bool,
        users: &Users,
    ) -> Result<()> {
        if self.migration_mode {
            if let Some(ref enhanced) = self.enhanced_bot {
                let bot = enhanced.lock().unwrap();
                return bot.process_message(
                    username,
                    content,
                    message_type,
                    message_id,
                    channel_source,
                    is_member,
                    users,
                );
            }
        }
        
        // Fallback to legacy bot with improved channel context
        let channel_context = match channel_source {
            BotChannel::Members => Some("members"),
            BotChannel::Staff => Some("staff"),
            BotChannel::Admin => Some("admin"),
            _ => None,
        };
        
        self.legacy_bot.process_message(
            username,
            content,
            message_type,
            message_id,
            channel_context,
            is_member,
        )?;
        
        Ok(())
    }
}

/// Enhanced channel detection logic
pub fn detect_message_channel(
    message_content: &str,
    message_context: &MessageContext,
    members_tag: &str,
    staffs_tag: &str,
) -> BotChannel {
    // This is the key fix: detect channel based on MESSAGE CONTEXT, not content scanning
    match message_context {
        MessageContext::MembersChannel => BotChannel::Members,
        MessageContext::StaffChannel => BotChannel::Staff,
        MessageContext::AdminChannel => BotChannel::Admin,
        MessageContext::PrivateMessage { to: _ } => BotChannel::Public, // PM context
        MessageContext::PublicChannel => BotChannel::Public,
        MessageContext::Unknown => {
            // Fallback: only use content detection as last resort
            if message_content.starts_with(members_tag) {
                BotChannel::Members
            } else if message_content.starts_with(staffs_tag) {
                BotChannel::Staff
            } else {
                BotChannel::Public
            }
        }
    }
}

/// Message context that should be determined by the message parser
#[derive(Debug, Clone)]
pub enum MessageContext {
    PublicChannel,
    MembersChannel,
    StaffChannel,
    AdminChannel,
    PrivateMessage { to: String },
    Unknown,
}

/// Fixed integration with main.rs message processing
pub fn process_bot_message_fixed(
    bot_integration: &BotIntegration,
    username: &str,
    content: &str,
    message_id: Option<u64>,
    users: &Users,
    message_context: MessageContext, // This should come from proper message parsing
    members_tag: &str,
    staffs_tag: &str,
) -> Result<()> {
    let channel_source = detect_message_channel(content, &message_context, members_tag, staffs_tag);
    let is_member = users.members.iter().any(|(_, name)| name == username);
    
    bot_integration.process_message_enhanced(
        username,
        content,
        MessageType::Normal,
        message_id,
        channel_source,
        is_member,
        users,
    )?;
    
    Ok(())
}

/// Utility function to determine message context from le-chat-php message structure
/// This should be called from the message parsing logic in main.rs
pub fn parse_message_context(
    message_html: &str,
    to_field: &Option<String>,
    from_user: &str,
    members_tag: &str,
    staffs_tag: &str,
) -> MessageContext {
    // If it's a private message
    if let Some(to) = to_field {
        return MessageContext::PrivateMessage { to: to.clone() };
    }
    
    // Parse the message structure to determine actual channel
    // In le-chat-php, channel context should be determined by:
    // 1. The sendto parameter used when the message was sent
    // 2. The message structure/formatting
    // 3. NOT by scanning the content for tags
    
    // This is where we need to examine the actual le-chat-php message format
    // to properly detect which channel a message came from
    
    // For now, provide basic detection until we can examine the message format
    if message_html.contains(&format!("sendto={}", "s ?")) {
        MessageContext::MembersChannel
    } else if message_html.contains(&format!("sendto={}", "s %")) {
        MessageContext::StaffChannel
    } else if message_html.contains(&format!("sendto={}", "s _")) {
        MessageContext::AdminChannel
    } else {
        // Default to public if we can't determine
        MessageContext::PublicChannel
    }
}

/// Enhanced command processing that integrates with ChatOps
pub fn process_enhanced_command(
    bot_integration: &BotIntegration,
    command: &str,
    args: &[String],
    username: &str,
    channel: BotChannel,
    is_admin: bool,
    chatops_router: Option<&ChatOpsRouter>,
) -> Result<Option<EnhancedBotResponse>> {
    // First try ChatOps integration for developer commands
    if let Some(chatops) = chatops_router {
        let user_role = if is_admin {
            UserRole::Admin
        } else {
            UserRole::Member
        };
        
        // Try to process as ChatOps command first
        if let Some(chatops_result) = chatops.process_command(
            &format!("/{} {}", command, args.join(" ")),
            username,
            user_role,
        ) {
            // Convert ChatOps result to bot response
            let messages = chatops_result.to_messages();
            if !messages.is_empty() {
                let response = EnhancedBotResponse::ChannelMessage {
                    content: messages.join("\n"),
                    channel: BotChannel::Current, // Respond in same channel
                };
                return Ok(Some(response));
            }
        }
    }
    
    // Then try enhanced bot commands
    if bot_integration.migration_mode {
        if let Some(ref enhanced) = bot_integration.enhanced_bot {
            // Enhanced bot command processing would go here
            // This integrates with the moderation and automation features
        }
    }
    
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_channel_detection() {
        let context = MessageContext::MembersChannel;
        let channel = detect_message_channel("test message", &context, "[M]", "[S]");
        assert!(matches!(channel, BotChannel::Members));
        
        let context = MessageContext::PublicChannel;
        let channel = detect_message_channel("[M] test message", &context, "[M]", "[S]");
        // Should NOT detect as members channel just because content has [M] tag
        assert!(matches!(channel, BotChannel::Public));
    }
    
    #[test]
    fn test_message_context_parsing() {
        let context = parse_message_context("", &None, "user", "[M]", "[S]");
        assert!(matches!(context, MessageContext::PublicChannel));
        
        let context = parse_message_context("", &Some("target".to_string()), "user", "[M]", "[S]");
        assert!(matches!(context, MessageContext::PrivateMessage { .. }));
    }
}