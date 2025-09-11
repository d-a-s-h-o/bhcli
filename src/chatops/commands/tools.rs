use crate::chatops::{ChatCommand, ChatOpError, ChatOpResult, CommandContext};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use rand::Rng;
use regex::Regex;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Hash generation command
pub struct HashCommand;

impl ChatCommand for HashCommand {
    fn name(&self) -> &'static str {
        "hash"
    }
    fn description(&self) -> &'static str {
        "Generate hash of text using various algorithms"
    }
    fn usage(&self) -> &'static str {
        "/hash <algorithm> <text>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.len() < 2 {
            return Err(ChatOpError::MissingArguments(
                "Please specify algorithm and text".to_string(),
            ));
        }

        let algorithm = args[0].to_lowercase();
        let text = args[1..].join(" ");

        let hash_result = match algorithm.as_str() {
            "md5" => {
                let digest = md5::compute(text.as_bytes());
                format!("{:x}", digest)
            }
            "sha1" => {
                use sha1::{Digest, Sha1};
                let mut hasher = Sha1::new();
                hasher.update(text.as_bytes());
                format!("{:x}", hasher.finalize())
            }
            "sha256" => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(text.as_bytes());
                format!("{:x}", hasher.finalize())
            }
            "sha512" => {
                use sha2::{Digest, Sha512};
                let mut hasher = Sha512::new();
                hasher.update(text.as_bytes());
                format!("{:x}", hasher.finalize())
            }
            _ => {
                return Err(ChatOpError::InvalidSyntax(
                    "Supported algorithms: md5, sha1, sha256, sha512".to_string(),
                ))
            }
        };

        Ok(ChatOpResult::Message(format!(
            "🔐 {} hash: `{}`",
            algorithm.to_uppercase(),
            hash_result
        )))
    }
}

/// UUID generation command
pub struct UuidCommand;

impl ChatCommand for UuidCommand {
    fn name(&self) -> &'static str {
        "uuid"
    }
    fn description(&self) -> &'static str {
        "Generate a random UUID"
    }
    fn usage(&self) -> &'static str {
        "/uuid"
    }

    fn execute(
        &self,
        _args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let uuid = Uuid::new_v4();
        Ok(ChatOpResult::Message(format!(
            "🆔 Generated UUID: `{}`",
            uuid
        )))
    }
}

/// Base64 encoding/decoding command
pub struct Base64Command;

impl ChatCommand for Base64Command {
    fn name(&self) -> &'static str {
        "base64"
    }
    fn description(&self) -> &'static str {
        "Encode or decode Base64 text"
    }
    fn usage(&self) -> &'static str {
        "/base64 <encode|decode> <text>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.len() < 2 {
            return Err(ChatOpError::MissingArguments(
                "Please specify operation (encode/decode) and text".to_string(),
            ));
        }

        let operation = args[0].to_lowercase();
        let text = args[1..].join(" ");

        match operation.as_str() {
            "encode" | "enc" => {
                let encoded = general_purpose::STANDARD.encode(text.as_bytes());
                Ok(ChatOpResult::Message(format!(
                    "🔤 Base64 encoded: `{}`",
                    encoded
                )))
            }
            "decode" | "dec" => match general_purpose::STANDARD.decode(&text) {
                Ok(decoded_bytes) => match String::from_utf8(decoded_bytes) {
                    Ok(decoded_text) => Ok(ChatOpResult::Message(format!(
                        "🔤 Base64 decoded: `{}`",
                        decoded_text
                    ))),
                    Err(_) => Ok(ChatOpResult::Message(
                        "🔤 Base64 decoded to binary data (not valid UTF-8)".to_string(),
                    )),
                },
                Err(_) => Err(ChatOpError::InvalidSyntax(
                    "Invalid Base64 input".to_string(),
                )),
            },
            _ => Err(ChatOpError::InvalidSyntax(
                "Operation must be 'encode' or 'decode'".to_string(),
            )),
        }
    }
}

/// Regex testing command
pub struct RegexCommand;

impl ChatCommand for RegexCommand {
    fn name(&self) -> &'static str {
        "regex"
    }
    fn description(&self) -> &'static str {
        "Test regex pattern against text"
    }
    fn usage(&self) -> &'static str {
        "/regex <pattern> <text>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["re"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.len() < 2 {
            return Err(ChatOpError::MissingArguments(
                "Please specify pattern and text".to_string(),
            ));
        }

        let pattern = &args[0];
        let text = args[1..].join(" ");

        match Regex::new(pattern) {
            Ok(re) => {
                let matches: Vec<_> = re.find_iter(&text).collect();

                if matches.is_empty() {
                    Ok(ChatOpResult::Message("🔍 No matches found".to_string()))
                } else {
                    let mut result = vec![format!("🔍 Found {} match(es):", matches.len())];

                    for (i, m) in matches.iter().enumerate().take(5) {
                        // Limit to 5 matches
                        result.push(format!(
                            "  {}. \"{}\" at position {}-{}",
                            i + 1,
                            m.as_str(),
                            m.start(),
                            m.end()
                        ));
                    }

                    if matches.len() > 5 {
                        result.push(format!("  ... and {} more", matches.len() - 5));
                    }

                    Ok(ChatOpResult::Block(result))
                }
            }
            Err(e) => Err(ChatOpError::InvalidSyntax(format!(
                "Invalid regex pattern: {}",
                e
            ))),
        }
    }
}

/// WHOIS lookup command
pub struct WhoisCommand;

impl ChatCommand for WhoisCommand {
    fn name(&self) -> &'static str {
        "whois"
    }
    fn description(&self) -> &'static str {
        "Perform WHOIS lookup on a domain"
    }
    fn usage(&self) -> &'static str {
        "/whois <domain>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify a domain".to_string(),
            ));
        }

        let domain = &args[0];

        match Command::new("whois").arg(domain).output() {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .filter(|line| !line.trim().is_empty() && !line.starts_with('%'))
                        .take(10) // Limit output
                        .map(|line| line.trim().to_string())
                        .collect();

                    if lines.is_empty() {
                        Ok(ChatOpResult::Message(format!(
                            "🌐 No WHOIS data found for '{}'",
                            domain
                        )))
                    } else {
                        Ok(ChatOpResult::Block(lines))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!(
                        "🌐 WHOIS lookup failed for '{}'",
                        domain
                    )))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!(
                "🌐 WHOIS command not available. Try: https://whois.net/whois/{}",
                domain
            ))),
        }
    }
}

/// DNS lookup command
pub struct DigCommand;

impl ChatCommand for DigCommand {
    fn name(&self) -> &'static str {
        "dig"
    }
    fn description(&self) -> &'static str {
        "Perform DNS lookup"
    }
    fn usage(&self) -> &'static str {
        "/dig <domain> [record_type]"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify a domain".to_string(),
            ));
        }

        let domain = &args[0];
        let record_type = args.get(1).map(|s| s.as_str()).unwrap_or("A");

        match Command::new("dig")
            .args(&["+short", domain, record_type])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| format!("🌐 {} {} {}", domain, record_type, line.trim()))
                        .collect();

                    if lines.is_empty() {
                        Ok(ChatOpResult::Message(format!("🌐 No {} records found for '{}'", record_type, domain)))
                    } else {
                        Ok(ChatOpResult::Block(lines))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!("🌐 DNS lookup failed for '{}'", domain)))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!("🌐 dig command not available. Try: https://www.nslookup.io/domains/{}/dns-records/", domain))),
        }
    }
}

/// IP information lookup command
pub struct IpInfoCommand;

impl ChatCommand for IpInfoCommand {
    fn name(&self) -> &'static str {
        "ipinfo"
    }
    fn description(&self) -> &'static str {
        "Get IP address information"
    }
    fn usage(&self) -> &'static str {
        "/ipinfo <ip_address>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["ip"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify an IP address".to_string(),
            ));
        }

        let ip = &args[0];

        // Basic IP validation
        if !ip.chars().all(|c| c.is_ascii_digit() || c == '.') || ip.split('.').count() != 4 {
            return Err(ChatOpError::InvalidSyntax(
                "Please provide a valid IPv4 address".to_string(),
            ));
        }

        // Try to get info from a free service
        match Command::new("curl")
            .args(&["-s", &format!("https://ipinfo.io/{}/json", ip)])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    // Simple parsing - in a real implementation you'd use serde_json
                    if result.contains("\"ip\"") {
                        Ok(ChatOpResult::CodeBlock(
                            result.to_string(),
                            Some("json".to_string()),
                        ))
                    } else {
                        Ok(ChatOpResult::Message(format!(
                            "🌍 No information found for IP: {}",
                            ip
                        )))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!(
                        "🌍 IP lookup failed for '{}'",
                        ip
                    )))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!(
                "🌍 IP lookup not available. Try: https://ipinfo.io/{}",
                ip
            ))),
        }
    }
}

/// Random number generator command
pub struct RandCommand;

impl ChatCommand for RandCommand {
    fn name(&self) -> &'static str {
        "rand"
    }
    fn description(&self) -> &'static str {
        "Generate random number"
    }
    fn usage(&self) -> &'static str {
        "/rand [min] [max]"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["random", "rng"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let mut rng = rand::thread_rng();

        match args.len() {
            0 => {
                // Random float between 0 and 1
                let num: f64 = rng.gen();
                Ok(ChatOpResult::Message(format!("🎲 Random: {:.6}", num)))
            }
            1 => {
                // Random int between 0 and max
                match args[0].parse::<i32>() {
                    Ok(max) if max > 0 => {
                        let num = rng.gen_range(0..=max);
                        Ok(ChatOpResult::Message(format!(
                            "🎲 Random (0-{}): {}",
                            max, num
                        )))
                    }
                    _ => Err(ChatOpError::InvalidSyntax(
                        "Max must be a positive integer".to_string(),
                    )),
                }
            }
            2 => {
                // Random int between min and max
                match (args[0].parse::<i32>(), args[1].parse::<i32>()) {
                    (Ok(min), Ok(max)) if min < max => {
                        let num = rng.gen_range(min..=max);
                        Ok(ChatOpResult::Message(format!(
                            "🎲 Random ({}-{}): {}",
                            min, max, num
                        )))
                    }
                    _ => Err(ChatOpError::InvalidSyntax(
                        "Min and max must be integers with min < max".to_string(),
                    )),
                }
            }
            _ => Err(ChatOpError::InvalidSyntax("Too many arguments".to_string())),
        }
    }
}

/// Time display command
pub struct TimeCommand;

impl ChatCommand for TimeCommand {
    fn name(&self) -> &'static str {
        "time"
    }
    fn description(&self) -> &'static str {
        "Show current time in UTC and local"
    }
    fn usage(&self) -> &'static str {
        "/time"
    }

    fn execute(
        &self,
        _args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let now = SystemTime::now();
        let utc: DateTime<Utc> = now.into();

        // Get local time zone if possible
        let local_info = match Command::new("date").output() {
            Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
            Err(_) => "Local time unavailable".to_string(),
        };

        let unix_timestamp = now
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let result = vec![
            "🕐 **Current Time:**".to_string(),
            format!("UTC: {}", utc.format("%Y-%m-%d %H:%M:%S UTC")),
            format!("Local: {}", local_info),
            format!("Unix Timestamp: {}", unix_timestamp),
        ];

        Ok(ChatOpResult::Block(result))
    }
}
