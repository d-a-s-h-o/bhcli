use crate::Users;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;

/// Represents the relationship status between master and alt accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountRelationshipStatus {
    /// Both accounts are online and linked
    Active,
    /// Master account is offline (may be in incognito mode)
    MasterOffline,
    /// Alt account is offline
    AltOffline,
    /// Both accounts are offline
    BothOffline,
    /// No relationship configured
    None,
}

/// Enhanced account management system
#[derive(Debug, Clone)]
pub struct AccountManager {
    /// Current username
    pub current_user: String,
    /// Master account username (if this is an alt)
    pub master_account: Option<String>,
    /// Alt account username (if this is a master)
    pub alt_account: Option<String>,
    /// Whether this instance is running as a master account
    pub is_master: bool,
    /// Last time accounts were verified as online together
    pub last_verified_together: RefCell<Option<DateTime<Utc>>>,
    /// Custom commands that can be executed by alt on behalf of master
    pub delegated_commands: HashMap<String, String>,
    /// Whether incognito mode detection is enabled
    pub incognito_detection_enabled: bool,
}

impl Default for AccountManager {
    fn default() -> Self {
        Self {
            current_user: String::new(),
            master_account: None,
            alt_account: None,
            is_master: false,
            last_verified_together: RefCell::new(None),
            delegated_commands: HashMap::new(),
            incognito_detection_enabled: true,
        }
    }
}

impl AccountManager {
    /// Create a new account manager
    pub fn new(current_user: String) -> Self {
        Self {
            current_user,
            ..Default::default()
        }
    }

    /// Set master account relationship
    pub fn set_master_account(&mut self, master: String) {
        self.master_account = Some(master);
        self.is_master = false;
        self.setup_default_delegated_commands();
    }

    /// Set alt account relationship
    pub fn set_alt_account(&mut self, alt: String) {
        self.alt_account = Some(alt);
        self.is_master = true;
        self.setup_default_delegated_commands();
    }

    /// Check the current relationship status
    pub fn get_relationship_status(&self, users: &Arc<Mutex<Users>>) -> AccountRelationshipStatus {
        if self.master_account.is_none() && self.alt_account.is_none() {
            return AccountRelationshipStatus::None;
        }

        let users = users.lock().unwrap();
        let all_users: Vec<String> = users.all().iter().map(|(_, name)| name.clone()).collect();

        match (&self.master_account, &self.alt_account) {
            (Some(master), None) => {
                // This is an alt account, check if master is online
                if all_users.contains(master) {
                    self.update_last_verified();
                    AccountRelationshipStatus::Active
                } else if self.incognito_detection_enabled {
                    // Master might be in incognito mode, check recent activity
                    if self.was_recently_verified() {
                        AccountRelationshipStatus::Active
                    } else {
                        AccountRelationshipStatus::MasterOffline
                    }
                } else {
                    AccountRelationshipStatus::MasterOffline
                }
            }
            (None, Some(alt)) => {
                // This is a master account, check if alt is online
                if all_users.contains(alt) {
                    self.update_last_verified();
                    AccountRelationshipStatus::Active
                } else {
                    AccountRelationshipStatus::AltOffline
                }
            }
            (Some(master), Some(_)) => {
                // Both are configured (shouldn't happen but handle gracefully)
                if all_users.contains(master) {
                    self.update_last_verified();
                    AccountRelationshipStatus::Active
                } else {
                    AccountRelationshipStatus::MasterOffline
                }
            }
            (None, None) => AccountRelationshipStatus::None,
        }
    }

    /// Check if accounts were recently verified together (for incognito mode detection)
    fn was_recently_verified(&self) -> bool {
        if let Some(last_verified) = *self.last_verified_together.borrow() {
            let now = Utc::now();
            let duration = now.signed_duration_since(last_verified);
            duration.num_minutes() < 10 // Consider active if verified within 10 minutes
        } else {
            false
        }
    }

    /// Update the last verified timestamp
    fn update_last_verified(&self) {
        *self.last_verified_together.borrow_mut() = Some(Utc::now());
    }

    /// Check if a command can be delegated from alt to master
    pub fn can_delegate_command(&self, command: &str) -> bool {
        if !self.is_relationship_active_cached() {
            return false;
        }

        // Allow basic commands and custom aliases
        self.delegated_commands.contains_key(command) || 
        self.is_safe_delegated_command(command)
    }

    /// Check if a command is safe for delegation without explicit configuration
    fn is_safe_delegated_command(&self, command: &str) -> bool {
        let safe_commands = [
            "pm", "kick", "k", "ban", "unban", "filter", "unfilter",
            "dl", "dall", "ignore", "unignore", "op", "deop",
            "voice", "devoice", "topic", "motd", "rules"
        ];
        
        // Check if it's a basic safe command
        if safe_commands.contains(&command) {
            return true;
        }

        // Check if it's a custom command (starting with !)
        if command.starts_with('!') {
            return true;
        }

        // Check if it's an alias command (custom user command)
        command.chars().all(|c| c.is_alphanumeric() || c == '_')
    }

    /// Execute a delegated command from alt to master
    pub fn execute_delegated_command(&self, command: &str, args: &[&str]) -> Option<String> {
        if !self.can_delegate_command(command) {
            return None;
        }

        match command {
            // Handle PM forwarding with enhanced context
            "pm" => {
                if args.len() >= 2 {
                    let target = args[0];
                    let message = args[1..].join(" ");
                    if self.master_account.is_some() {
                        Some(format!("/pm {} [via {}] {}", target, self.current_user, message))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            // Handle moderation commands
            "kick" | "k" => {
                if !args.is_empty() {
                    let target = args[0];
                    let reason = if args.len() > 1 { 
                        format!(" {}", args[1..].join(" ")) 
                    } else { 
                        String::new() 
                    };
                    if let Some(master) = &self.master_account {
                        Some(format!("/pm {} #kick {}{} (requested by {})", master, target, reason, self.current_user))
                    } else {
                        Some(format!("/{} {}{}", command, target, reason))
                    }
                } else {
                    None
                }
            }

            "ban" => {
                if !args.is_empty() {
                    let target = args[0];
                    let reason = if args.len() > 1 { 
                        format!(" {}", args[1..].join(" ")) 
                    } else { 
                        String::new() 
                    };
                    if let Some(master) = &self.master_account {
                        Some(format!("/pm {} #ban {}{} (requested by {})", master, target, reason, self.current_user))
                    } else {
                        Some(format!("/ban {}{}", target, reason))
                    }
                } else {
                    None
                }
            }

            // Handle custom delegated commands
            _ => {
                if let Some(template) = self.delegated_commands.get(command) {
                    let mut result = template.clone();
                    // Replace placeholders with arguments
                    for (i, arg) in args.iter().enumerate() {
                        result = result.replace(&format!("{{{}}}", i), arg);
                    }
                    Some(result)
                } else if command.starts_with('!') {
                    // Custom user command - execute directly
                    Some(command.to_string())
                } else {
                    // Try to execute as direct command
                    let full_command = if args.is_empty() {
                        format!("/{}", command)
                    } else {
                        format!("/{} {}", command, args.join(" "))
                    };
                    Some(full_command)
                }
            }
        }
    }

    /// Set up default delegated commands
    fn setup_default_delegated_commands(&mut self) {
        self.delegated_commands.clear();
        
        // Add some useful default templates
        self.delegated_commands.insert(
            "warn".to_string(),
            "/pm {0} This is your warning @{0}, will be kicked next !rules".to_string()
        );
        
        self.delegated_commands.insert(
            "welcome".to_string(),
            "Welcome to the chat @{0}! Please read the !rules".to_string()
        );

        self.delegated_commands.insert(
            "op".to_string(),
            "/op {0}".to_string()
        );

        self.delegated_commands.insert(
            "deop".to_string(),
            "/deop {0}".to_string()
        );
    }

    /// Get the related account name
    pub fn get_related_account(&self) -> Option<&String> {
        self.master_account.as_ref().or(self.alt_account.as_ref())
    }

    /// Check if relationship is currently active (cached version for const contexts)
    fn is_relationship_active_cached(&self) -> bool {
        // This is a simplified check - in practice, you'd want to cache the last status
        // For now, assume active if relationship exists and was recently verified
        self.get_related_account().is_some() && self.was_recently_verified()
    }

    /// Format a status message for display
    pub fn format_status_message(&self, status: &AccountRelationshipStatus) -> String {
        match status {
            AccountRelationshipStatus::Active => {
                if let Some(related) = self.get_related_account() {
                    if self.is_master {
                        format!("🔗 Master account linked to alt: {} (Active)", related)
                    } else {
                        format!("🔗 Alt account linked to master: {} (Active)", related)
                    }
                } else {
                    "🔗 Account relationship active".to_string()
                }
            }
            AccountRelationshipStatus::MasterOffline => {
                if let Some(master) = &self.master_account {
                    format!("⚠️ Master account {} appears offline (may be incognito)", master)
                } else {
                    "⚠️ Master account offline".to_string()
                }
            }
            AccountRelationshipStatus::AltOffline => {
                if let Some(alt) = &self.alt_account {
                    format!("⚠️ Alt account {} is offline", alt)
                } else {
                    "⚠️ Alt account offline".to_string()
                }
            }
            AccountRelationshipStatus::BothOffline => {
                "❌ Both master and alt accounts are offline".to_string()
            }
            AccountRelationshipStatus::None => {
                "No master/alt relationship configured".to_string()
            }
        }
    }
}

/// Enhanced command parsing that handles master/alt delegation
pub fn parse_enhanced_command(
    input: &str, 
    account_manager: &AccountManager
) -> Option<String> {
    if input.starts_with('/') {
        let parts: Vec<&str> = input[1..].split_whitespace().collect();
        if !parts.is_empty() {
            let command = parts[0];
            let args: Vec<&str> = parts[1..].iter().cloned().collect();
            
            // Check if this command can be delegated
            if account_manager.can_delegate_command(command) {
                return account_manager.execute_delegated_command(command, &args);
            }
        }
    }
    
    // Return original input if no delegation needed
    Some(input.to_string())
}