use crate::chatops::{ChatCommand, ChatOpError, ChatOpResult, CommandContext};
use std::process::Command;

/// GitHub repository information
pub struct GitHubCommand;

impl ChatCommand for GitHubCommand {
    fn name(&self) -> &'static str {
        "github"
    }
    fn description(&self) -> &'static str {
        "Get GitHub repository information"
    }
    fn usage(&self) -> &'static str {
        "/github <user>/<repo> [issues|latest|file <path>]"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["gh"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify a repository (user/repo)".to_string(),
            ));
        }

        let repo = &args[0];
        if !repo.contains('/') {
            return Err(ChatOpError::InvalidSyntax(
                "Repository must be in format 'user/repo'".to_string(),
            ));
        }

        let action = args.get(1).map(|s| s.as_str()).unwrap_or("info");

        match action {
            "issues" => Ok(ChatOpResult::Message(format!(
                "🐛 GitHub Issues for {}: https://github.com/{}/issues",
                repo, repo
            ))),
            "latest" => Ok(ChatOpResult::Message(format!(
                "🏷️ Latest Release for {}: https://github.com/{}/releases/latest",
                repo, repo
            ))),
            "file" => {
                if args.len() < 3 {
                    return Err(ChatOpError::MissingArguments(
                        "Please specify file path".to_string(),
                    ));
                }
                let file_path = &args[2];
                Ok(ChatOpResult::Message(format!(
                    "📄 File {}: https://github.com/{}/blob/main/{}",
                    file_path, repo, file_path
                )))
            }
            _ => {
                // Basic repo info
                let info = vec![
                    format!("📦 **GitHub Repository: {}**", repo),
                    format!("🔗 URL: https://github.com/{}", repo),
                    format!("📊 Issues: https://github.com/{}/issues", repo),
                    format!("🏷️ Releases: https://github.com/{}/releases", repo),
                    format!("📋 README: https://github.com/{}#readme", repo),
                ];
                Ok(ChatOpResult::Block(info))
            }
        }
    }
}

/// GitHub Gist creation
pub struct GistCommand;

impl ChatCommand for GistCommand {
    fn name(&self) -> &'static str {
        "gist"
    }
    fn description(&self) -> &'static str {
        "Create a GitHub Gist (requires gh CLI)"
    }
    fn usage(&self) -> &'static str {
        "/gist <code>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify code to create a gist".to_string(),
            ));
        }

        let code = args.join(" ");

        // Try using GitHub CLI if available
        match Command::new("gh")
            .args(&["gist", "create", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                use std::io::Write;
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(code.as_bytes());
                }

                match child.wait_with_output() {
                    Ok(output) => {
                        if output.status.success() {
                            let gist_url =
                                String::from_utf8_lossy(&output.stdout).trim().to_string();
                            Ok(ChatOpResult::Message(format!(
                                "📝 Gist created: {}",
                                gist_url
                            )))
                        } else {
                            Ok(ChatOpResult::Message("📝 Failed to create gist. Make sure you're logged in with `gh auth login`".to_string()))
                        }
                    }
                    Err(_) => Ok(ChatOpResult::Message(
                        "📝 Failed to create gist".to_string(),
                    )),
                }
            }
            Err(_) => {
                // Fallback - just show the manual gist creation URL
                Ok(ChatOpResult::Message(format!(
                    "📝 Create gist manually at: https://gist.github.com/"
                )))
            }
        }
    }
}

/// Rust crates.io information
pub struct CratesCommand;

impl ChatCommand for CratesCommand {
    fn name(&self) -> &'static str {
        "crates"
    }
    fn description(&self) -> &'static str {
        "Get Rust crate information from crates.io"
    }
    fn usage(&self) -> &'static str {
        "/crates <crate_name>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify a crate name".to_string(),
            ));
        }

        let crate_name = &args[0];

        // Try to fetch from crates.io API
        match Command::new("curl")
            .args(&[
                "-s",
                &format!("https://crates.io/api/v1/crates/{}", crate_name),
            ])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    if result.contains("\"crate\"") {
                        // Parse basic info (in real implementation, use serde_json)
                        let info = vec![
                            format!("📦 **Rust Crate: {}**", crate_name),
                            format!("🔗 crates.io: https://crates.io/crates/{}", crate_name),
                            format!("📚 docs.rs: https://docs.rs/{}", crate_name),
                            format!("📋 Add to Cargo.toml: {} = \"latest\"", crate_name),
                        ];
                        Ok(ChatOpResult::Block(info))
                    } else {
                        Ok(ChatOpResult::Message(format!(
                            "📦 Crate '{}' not found on crates.io",
                            crate_name
                        )))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!(
                        "📦 Failed to fetch info for crate '{}'",
                        crate_name
                    )))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!(
                "📦 Check crate manually: https://crates.io/crates/{}",
                crate_name
            ))),
        }
    }
}

/// NPM package information
pub struct NpmCommand;

impl ChatCommand for NpmCommand {
    fn name(&self) -> &'static str {
        "npm"
    }
    fn description(&self) -> &'static str {
        "Get NPM package information"
    }
    fn usage(&self) -> &'static str {
        "/npm <package_name>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify a package name".to_string(),
            ));
        }

        let package_name = &args[0];

        // Try using npm view command
        match Command::new("npm")
            .args(&["view", package_name, "--json"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    if result.contains("\"name\"") {
                        let info = vec![
                            format!("📦 **NPM Package: {}**", package_name),
                            format!(
                                "🔗 npmjs.com: https://www.npmjs.com/package/{}",
                                package_name
                            ),
                            format!("📋 Install: npm install {}", package_name),
                            format!("📋 Or: yarn add {}", package_name),
                        ];
                        Ok(ChatOpResult::Block(info))
                    } else {
                        Ok(ChatOpResult::Message(format!(
                            "📦 Package '{}' not found on NPM",
                            package_name
                        )))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!(
                        "📦 Failed to fetch info for package '{}'",
                        package_name
                    )))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!(
                "📦 Check package manually: https://www.npmjs.com/package/{}",
                package_name
            ))),
        }
    }
}

/// Python PyPI package information
pub struct PipCommand;

impl ChatCommand for PipCommand {
    fn name(&self) -> &'static str {
        "pip"
    }
    fn description(&self) -> &'static str {
        "Get Python package information from PyPI"
    }
    fn usage(&self) -> &'static str {
        "/pip <package_name>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["pypi"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify a package name".to_string(),
            ));
        }

        let package_name = &args[0];

        // Try to fetch from PyPI API
        match Command::new("curl")
            .args(&[
                "-s",
                &format!("https://pypi.org/pypi/{}/json", package_name),
            ])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    if result.contains("\"info\"") && !result.contains("\"message\": \"Not Found\"")
                    {
                        let info = vec![
                            format!("🐍 **Python Package: {}**", package_name),
                            format!("🔗 PyPI: https://pypi.org/project/{}/", package_name),
                            format!("📋 Install: pip install {}", package_name),
                            format!("📋 Or: python -m pip install {}", package_name),
                        ];
                        Ok(ChatOpResult::Block(info))
                    } else {
                        Ok(ChatOpResult::Message(format!(
                            "🐍 Package '{}' not found on PyPI",
                            package_name
                        )))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!(
                        "🐍 Failed to fetch info for package '{}'",
                        package_name
                    )))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!(
                "🐍 Check package manually: https://pypi.org/project/{}/",
                package_name
            ))),
        }
    }
}
