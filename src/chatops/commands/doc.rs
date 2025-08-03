use crate::chatops::{ChatCommand, CommandContext, ChatOpResult, ChatOpError};
use std::process::Command;

/// Manual page lookup command
pub struct ManCommand;

impl ChatCommand for ManCommand {
    fn name(&self) -> &'static str { "man" }
    fn description(&self) -> &'static str { "Get manual page summary for a command" }
    fn usage(&self) -> &'static str { "/man <command>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a command".to_string()));
        }
        
        let command = &args[0];
        
        // Try to get man page using `man` command
        let match_result = match Command::new("man")
            .args(&["-f", command])  // whatis format - brief description
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    if result.trim().is_empty() {
                        ChatOpResult::Message(format!("📖 No manual entry found for '{}'", command))
                    } else {
                        let lines: Vec<String> = result
                            .lines()
                            .take(5) // Limit to first 5 results
                            .map(|line| format!("📖 {}", line.trim()))
                            .collect();
                        ChatOpResult::Block(lines)
                    }
                } else {
                    ChatOpResult::Message(format!("📖 No manual entry found for '{}'", command))
                }
            }
            Err(_) => {
                // Fallback with some common commands
                let description = match command.as_str() {
                    "curl" => "transfer a URL - command line tool for transferring data",
                    "grep" => "print lines that match patterns",
                    "awk" => "pattern scanning and processing language",
                    "sed" => "stream editor for filtering and transforming text",
                    "tmux" => "terminal multiplexer",
                    "ssh" => "OpenSSH SSH client (remote login program)",
                    "git" => "the stupid content tracker",
                    "docker" => "A self-sufficient runtime for containers",
                    "vim" => "Vi IMproved, a programmer's text editor",
                    "ls" => "list directory contents",
                    "cd" => "change directory",
                    "cat" => "concatenate files and print on the standard output",
                    "tail" => "output the last part of files",
                    "head" => "output the first part of files",
                    _ => return Ok(ChatOpResult::Error(format!("No manual entry found for '{}' and man command not available", command))),
                };
                ChatOpResult::Message(format!("📖 {} - {}", command, description))
            }
        };
        Ok(match_result)
    }
}

/// Language-specific documentation lookup
pub struct DocCommand;

impl ChatCommand for DocCommand {
    fn name(&self) -> &'static str { "doc" }
    fn description(&self) -> &'static str { "Get language-specific documentation" }
    fn usage(&self) -> &'static str { "/doc <language> <term>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.len() < 2 {
            return Err(ChatOpError::MissingArguments("Please specify language and term".to_string()));
        }
        
        let language = args[0].to_lowercase();
        let term = &args[1];
        
        let doc_url = match language.as_str() {
            "rust" => format!("https://doc.rust-lang.org/std/?search={}", term),
            "python" | "py" => format!("https://docs.python.org/3/search.html?q={}", term),
            "javascript" | "js" => format!("https://developer.mozilla.org/en-US/search?q={}", term),
            "c" => format!("https://en.cppreference.com/w/c?search={}", term),
            "cpp" | "c++" => format!("https://en.cppreference.com/w/cpp?search={}", term),
            "go" => format!("https://pkg.go.dev/search?q={}", term),
            "java" => format!("https://docs.oracle.com/en/java/javase/17/docs/api/index.html?search={}", term),
            _ => return Err(ChatOpError::InvalidSyntax(format!("Unsupported language: {}", language))),
        };
        
        Ok(ChatOpResult::Message(format!("📚 {} docs for '{}': {}", language, term, doc_url)))
    }
}

/// Concept explanation command
pub struct ExplainCommand;

impl ChatCommand for ExplainCommand {
    fn name(&self) -> &'static str { "explain" }
    fn description(&self) -> &'static str { "Explain programming concepts" }
    fn usage(&self) -> &'static str { "/explain <concept>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a concept to explain".to_string()));
        }
        
        let concept = args.join(" ").to_lowercase();
        
        let explanation = match concept.as_str() {
            "async" | "asynchronous" => {
                "🔄 **Asynchronous Programming**: A programming paradigm that allows code to run concurrently without blocking. Operations can be initiated and then the program can continue executing other code while waiting for the operation to complete."
            }
            "regex" | "regular expression" => {
                "🔍 **Regular Expressions**: Patterns used to match character combinations in strings. Useful for searching, extracting, and replacing text based on patterns."
            }
            "jwt" | "json web token" => {
                "🔐 **JWT (JSON Web Token)**: A compact, URL-safe means of representing claims between two parties. Used for authentication and secure information transmission."
            }
            "rest api" | "restful" => {
                "🌐 **REST API**: Representational State Transfer - an architectural style for designing networked applications using standard HTTP methods (GET, POST, PUT, DELETE)."
            }
            "docker" => {
                "🐳 **Docker**: A containerization platform that packages applications with their dependencies into lightweight, portable containers."
            }
            "git" => {
                "📝 **Git**: A distributed version control system for tracking changes in source code during software development."
            }
            "mutex" | "mutual exclusion" => {
                "🔒 **Mutex**: A synchronization primitive that prevents multiple threads from accessing shared data simultaneously, preventing race conditions."
            }
            "blockchain" => {
                "⛓️ **Blockchain**: A distributed ledger technology that maintains a continuously growing list of records (blocks) linked using cryptography."
            }
            _ => {
                return Ok(ChatOpResult::Message(format!("🤔 I don't have an explanation for '{}' yet. Try searching online or ask a more specific question.", concept)));
            }
        };
        
        Ok(ChatOpResult::Message(explanation.to_string()))
    }
}

/// Cheat sheet lookup command
pub struct CheatCommand;

impl ChatCommand for CheatCommand {
    fn name(&self) -> &'static str { "cheat" }
    fn description(&self) -> &'static str { "Get cheat sheets from cht.sh" }
    fn usage(&self) -> &'static str { "/cheat <term>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a term for the cheat sheet".to_string()));
        }
        
        let term = args.join("+");
        
        // Try to fetch from cht.sh with plain text output and timeout
        match Command::new("curl")
            .args(&["-s", "--max-time", "5", &format!("https://cht.sh/{}?T", term)])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .take(20) // Limit to prevent spam but allow context
                        .map(|line| line.to_string())
                        .collect();
                    
                    if lines.is_empty() || lines[0].contains("Unknown") {
                        Ok(ChatOpResult::Message(format!("📋 No cheat sheet found for '{}'", args.join(" "))))
                    } else {
                        Ok(ChatOpResult::CodeBlock(lines.join("\n"), Some("text".to_string())))
                    }
                } else {
                    Err(ChatOpError::NetworkError("Failed to fetch cheat sheet".to_string()))
                }
            }
            Err(_) => {
                // Fallback message
                Ok(ChatOpResult::Message(format!("📋 Try: https://cht.sh/{}", term)))
            }
        }
    }
}

/// StackOverflow search command
pub struct StackOverflowCommand;

impl ChatCommand for StackOverflowCommand {
    fn name(&self) -> &'static str { "so" }
    fn description(&self) -> &'static str { "Search StackOverflow" }
    fn usage(&self) -> &'static str { "/so <query>" }
    fn aliases(&self) -> Vec<&'static str> { vec!["stackoverflow"] }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a search query".to_string()));
        }
        
        let query = args.join(" ");
        let encoded_query = query.replace(" ", "+");
        let url = format!("https://stackoverflow.com/search?q={}", encoded_query);
        
        Ok(ChatOpResult::Message(format!("🟠 StackOverflow search for '{}': {}", query, url)))
    }
}

/// Reference links command
pub struct RefCommand;

impl ChatCommand for RefCommand {
    fn name(&self) -> &'static str { "ref" }
    fn description(&self) -> &'static str { "Get official reference links" }
    fn usage(&self) -> &'static str { "/ref <library/language>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a library or language".to_string()));
        }
        
        let library = args[0].to_lowercase();
        
        let reference = match library.as_str() {
            "rust" => ("🦀 Rust", "https://doc.rust-lang.org/std/"),
            "python" => ("🐍 Python", "https://docs.python.org/3/"),
            "javascript" | "js" => ("📜 JavaScript", "https://developer.mozilla.org/en-US/docs/Web/JavaScript"),
            "react" => ("⚛️ React", "https://reactjs.org/docs/"),
            "vue" => ("💚 Vue.js", "https://vuejs.org/guide/"),
            "node" | "nodejs" => ("🟢 Node.js", "https://nodejs.org/en/docs/"),
            "docker" => ("🐳 Docker", "https://docs.docker.com/"),
            "git" => ("📝 Git", "https://git-scm.com/docs"),
            "linux" => ("🐧 Linux", "https://man7.org/linux/man-pages/"),
            "postgresql" | "postgres" => ("🐘 PostgreSQL", "https://www.postgresql.org/docs/"),
            "mysql" => ("🐬 MySQL", "https://dev.mysql.com/doc/"),
            "redis" => ("🔴 Redis", "https://redis.io/documentation"),
            _ => return Ok(ChatOpResult::Message(format!("🔍 No reference found for '{}'. Try a web search instead.", library))),
        };
        
        Ok(ChatOpResult::Message(format!("{} Reference: {}", reference.0, reference.1)))
    }
}
