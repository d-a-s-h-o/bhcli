use std::collections::HashMap;
use crate::chatops::{ChatCommand, CommandContext, UserRole, ChatOpResult, ChatOpError};

/// Registry for managing ChatOps commands
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn ChatCommand>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }
    
    /// Register a new command
    pub fn register(&mut self, command: Box<dyn ChatCommand>) {
        let name = command.name().to_string();
        
        // Register aliases
        for alias in command.aliases() {
            self.aliases.insert(alias.to_string(), name.clone());
        }
        
        self.commands.insert(name, command);
    }
    
    /// Get command by name or alias
    pub fn get_command(&self, name: &str) -> Option<&dyn ChatCommand> {
        let actual_name = name.to_string();
        let command_name = self.aliases.get(name).unwrap_or(&actual_name);
        self.commands.get(command_name).map(|cmd| cmd.as_ref())
    }
    
    /// Execute a command with arguments
    pub fn execute_command(
        &self,
        name: &str,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        match self.get_command(name) {
            Some(command) => {
                // Check permissions
                if !self.check_permission(&context.role, &command.required_role()) {
                    return Err(ChatOpError::PermissionDenied(
                        format!("Command '{}' requires {:?} role or higher", name, command.required_role())
                    ));
                }
                
                command.execute(args, context)
            }
            None => Err(ChatOpError::Generic(format!("Unknown command: {}", name))),
        }
    }
    
    /// List all available commands for a user role
    pub fn list_commands(&self, role: &UserRole) -> Vec<(&str, &str)> {
        self.commands
            .values()
            .filter(|cmd| self.check_permission(role, &cmd.required_role()))
            .map(|cmd| (cmd.name(), cmd.description()))
            .collect()
    }
    
    /// Get help for a specific command
    pub fn get_help(&self, name: &str) -> Option<String> {
        self.get_command(name).map(|cmd| {
            format!(
                "**{}** - {}\n\nUsage: {}\n\nAliases: {}",
                cmd.name(),
                cmd.description(),
                cmd.usage(),
                if cmd.aliases().is_empty() {
                    "none".to_string()
                } else {
                    cmd.aliases().join(", ")
                }
            )
        })
    }
    
    /// Check if user role has permission for required role
    fn check_permission(&self, user_role: &UserRole, required_role: &UserRole) -> bool {
        let user_level = self.role_level(user_role);
        let required_level = self.role_level(required_role);
        user_level >= required_level
    }
    
    /// Convert role to numeric level for comparison
    fn role_level(&self, role: &UserRole) -> u8 {
        match role {
            UserRole::Guest => 0,
            UserRole::Member => 1,
            UserRole::Staff => 2,
            UserRole::Admin => 3,
        }
    }
    
    /// Register a user alias for a command
    #[allow(dead_code)]
    pub fn register_alias(&mut self, alias: String, target: String) {
        if self.commands.contains_key(&target) {
            self.aliases.insert(alias, target);
        }
    }
    
    /// Remove a user alias
    #[allow(dead_code)]
    pub fn remove_alias(&mut self, alias: &str) {
        self.aliases.remove(alias);
    }
}
