//! Note management commands for BHCLI
//!
//! Provides functionality to view and edit notes in le-chat-php systems.
//! Supports personal, public, staff, and admin notes based on user permissions.

use crate::chatops::{ChatCommand, ChatOpError, ChatOpResult, CommandContext, UserRole};

/// Command for viewing notes
pub struct ViewNotesCommand;

impl ChatCommand for ViewNotesCommand {
    fn name(&self) -> &'static str {
        "viewnotes"
    }

    fn description(&self) -> &'static str {
        "View notes (personal, public, staff, admin)"
    }

    fn usage(&self) -> &'static str {
        "/viewnotes [personal|public|staff|admin|viewpublic]"
    }

    fn execute(
        &self,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let note_type = args.get(0).map(|s| s.as_str()).unwrap_or("personal");

        // Validate permissions
        match note_type {
            "personal" | "" => {
                if context.role == UserRole::Guest || context.role == UserRole::Member {
                    return Err(ChatOpError::PermissionDenied("Insufficient privileges for personal notes".to_string()));
                }
            }
            "public" => {
                if context.role == UserRole::Guest || context.role == UserRole::Member {
                    return Err(ChatOpError::PermissionDenied("Insufficient privileges for public notes".to_string()));
                }
            }
            "staff" => {
                if context.role != UserRole::Staff && context.role != UserRole::Admin {
                    return Err(ChatOpError::PermissionDenied("Staff or Admin role required for staff notes".to_string()));
                }
            }
            "admin" => {
                if context.role != UserRole::Admin {
                    return Err(ChatOpError::PermissionDenied("Admin role required for admin notes".to_string()));
                }
            }
            "viewpublic" => {} // No special permissions needed
            _ => {
                return Err(ChatOpError::InvalidSyntax(
                    "Invalid note type. Use: personal, public, staff, admin, or viewpublic".to_string(),
                ))
            }
        }

        // For now, return a message indicating that the server integration is needed
        Ok(ChatOpResult::Message(format!(
            "Note viewing feature requires server integration.\nType: {}\nRequested by: {}\n\nThis feature will use the active chat session to fetch notes from the server.",
            note_type,
            context.username
        )))
    }

    fn required_role(&self) -> UserRole {
        UserRole::Member
    }
}

/// Command for editing notes
pub struct EditNotesCommand;

impl ChatCommand for EditNotesCommand {
    fn name(&self) -> &'static str {
        "editnotes"
    }

    fn description(&self) -> &'static str {
        "Edit notes (personal, public, staff, admin)"
    }

    fn usage(&self) -> &'static str {
        "/editnotes [personal|public|staff|admin] <text>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Usage: /editnotes [type] <text>".to_string(),
            ));
        }

        let (note_type, text) = if args.len() == 1 {
            // Default to personal notes if only text is provided
            ("personal".to_string(), args[0].clone())
        } else {
            let note_type = args[0].as_str();
            let text = args[1..].join(" ");
            
            match note_type {
                "personal" => {
                    if context.role == UserRole::Guest || context.role == UserRole::Member {
                        return Err(ChatOpError::PermissionDenied("Insufficient privileges for personal notes".to_string()));
                    }
                    ("personal".to_string(), text)
                }
                "public" => {
                    if context.role == UserRole::Guest || context.role == UserRole::Member {
                        return Err(ChatOpError::PermissionDenied("Insufficient privileges for public notes".to_string()));
                    }
                    ("public".to_string(), text)
                }
                "staff" => {
                    if context.role != UserRole::Staff && context.role != UserRole::Admin {
                        return Err(ChatOpError::PermissionDenied("Staff or Admin role required for staff notes".to_string()));
                    }
                    ("staff".to_string(), text)
                }
                "admin" => {
                    if context.role != UserRole::Admin {
                        return Err(ChatOpError::PermissionDenied("Admin role required for admin notes".to_string()));
                    }
                    ("admin".to_string(), text)
                }
                _ => {
                    // Treat first arg as part of text for personal notes
                    if context.role == UserRole::Guest || context.role == UserRole::Member {
                        return Err(ChatOpError::PermissionDenied("Insufficient privileges for personal notes".to_string()));
                    }
                    ("personal".to_string(), args.join(" "))
                }
            }
        };

        // For now, return a message indicating that the server integration is needed
        Ok(ChatOpResult::Message(format!(
            "Note editing feature requires server integration.\nType: {}\nText: {}\nRequested by: {}\n\nThis feature will use the active chat session to save notes to the server.",
            note_type,
            text,
            context.username
        )))
    }

    fn required_role(&self) -> UserRole {
        UserRole::Member
    }
}