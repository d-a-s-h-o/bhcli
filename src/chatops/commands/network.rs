use crate::chatops::{ChatCommand, CommandContext, ChatOpResult, ChatOpError};
use std::process::Command;

/// Ping command
pub struct PingCommand;

impl ChatCommand for PingCommand {
    fn name(&self) -> &'static str { "ping" }
    fn description(&self) -> &'static str { "Ping a host" }
    fn usage(&self) -> &'static str { "/ping <host>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a host to ping".to_string()));
        }
        
        let host = &args[0];
        
        match Command::new("ping")
            .args(&["-c", "3", host]) // 3 packets on Unix
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .filter(|line| line.contains("time=") || line.contains("packet loss") || line.contains("min/avg/max"))
                        .take(5)
                        .map(|line| format!("🏓 {}", line.trim()))
                        .collect();
                    
                    if lines.is_empty() {
                        Ok(ChatOpResult::Message(format!("🏓 Ping to {} completed", host)))
                    } else {
                        Ok(ChatOpResult::Block(lines))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!("🏓 Ping to {} failed", host)))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!("🏓 Ping command not available"))),
        }
    }
}

/// Traceroute command
pub struct TraceCommand;

impl ChatCommand for TraceCommand {
    fn name(&self) -> &'static str { "trace" }
    fn description(&self) -> &'static str { "Trace route to host" }
    fn usage(&self) -> &'static str { "/trace <host>" }
    fn aliases(&self) -> Vec<&'static str> { vec!["traceroute"] }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a host to trace".to_string()));
        }
        
        let host = &args[0];
        
        match Command::new("traceroute")
            .args(&["-m", "10", host]) // Max 10 hops
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .take(12) // Limit output
                        .map(|line| format!("📍 {}", line.trim()))
                        .collect();
                    
                    if lines.is_empty() {
                        Ok(ChatOpResult::Message(format!("📍 Traceroute to {} completed", host)))
                    } else {
                        Ok(ChatOpResult::Block(lines))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!("📍 Traceroute to {} failed", host)))
                }
            }
            Err(_) => {
                // Try with tracepath as alternative
                match Command::new("tracepath").arg(host).output() {
                    Ok(output) if output.status.success() => {
                        let result = String::from_utf8_lossy(&output.stdout);
                        let lines: Vec<String> = result
                            .lines()
                            .take(12)
                            .map(|line| format!("📍 {}", line.trim()))
                            .collect();
                        Ok(ChatOpResult::Block(lines))
                    }
                    _ => Ok(ChatOpResult::Message("📍 Traceroute command not available".to_string())),
                }
            }
        }
    }
}

/// Port scan command
pub struct PortScanCommand;

impl ChatCommand for PortScanCommand {
    fn name(&self) -> &'static str { "portscan" }
    fn description(&self) -> &'static str { "Simple port scan (TCP connect)" }
    fn usage(&self) -> &'static str { "/portscan <host> [ports]" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a host to scan".to_string()));
        }
        
        let host = &args[0];
        let ports = args.get(1).unwrap_or(&"22,80,443".to_string()).clone();
        
        // Simple TCP connect scan using netcat if available
        match Command::new("nc")
            .args(&["-z", "-v", "-w", "2", host, &ports])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let result = format!("{}{}", stdout, stderr);
                
                let lines: Vec<String> = result
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .take(10)
                    .map(|line| format!("🔍 {}", line.trim()))
                    .collect();
                
                if lines.is_empty() {
                    Ok(ChatOpResult::Message(format!("🔍 Port scan of {} completed (no open ports found)", host)))
                } else {
                    Ok(ChatOpResult::Block(lines))
                }
            }
            Err(_) => {
                // Try with nmap if available
                match Command::new("nmap")
                    .args(&["-p", &ports, host])
                    .output()
                {
                    Ok(output) if output.status.success() => {
                        let result = String::from_utf8_lossy(&output.stdout);
                        let lines: Vec<String> = result
                            .lines()
                            .filter(|line| line.contains("/tcp") || line.contains("Nmap scan report"))
                            .take(10)
                            .map(|line| format!("🔍 {}", line.trim()))
                            .collect();
                        Ok(ChatOpResult::Block(lines))
                    }
                    _ => Ok(ChatOpResult::Message("🔍 Port scanning tools not available (nc/nmap)".to_string())),
                }
            }
        }
    }
}

/// HTTP headers command
pub struct HeadersCommand;

impl ChatCommand for HeadersCommand {
    fn name(&self) -> &'static str { "headers" }
    fn description(&self) -> &'static str { "Show HTTP response headers" }
    fn usage(&self) -> &'static str { "/headers <url>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a URL".to_string()));
        }
        
        let url = &args[0];
        
        match Command::new("curl")
            .args(&["-I", "-s", url])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .take(15) // Limit headers
                        .map(|line| format!("📡 {}", line.trim()))
                        .collect();
                    
                    if lines.is_empty() {
                        Ok(ChatOpResult::Message(format!("📡 No headers received from {}", url)))
                    } else {
                        Ok(ChatOpResult::Block(lines))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!("📡 Failed to fetch headers from {}", url)))
                }
            }
            Err(_) => Ok(ChatOpResult::Message("📡 curl command not available".to_string())),
        }
    }
}

/// Simple HTTP request command
pub struct CurlCommand;

impl ChatCommand for CurlCommand {
    fn name(&self) -> &'static str { "curl" }
    fn description(&self) -> &'static str { "Make HTTP request and show response" }
    fn usage(&self) -> &'static str { "/curl <url>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a URL".to_string()));
        }
        
        let url = &args[0];
        
        match Command::new("curl")
            .args(&["-s", "-L", "--max-time", "10", url])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = result
                        .lines()
                        .take(20) // Limit response body
                        .map(|line| line.to_string())
                        .collect();
                    
                    if lines.is_empty() {
                        Ok(ChatOpResult::Message(format!("🌐 Empty response from {}", url)))
                    } else {
                        Ok(ChatOpResult::CodeBlock(lines.join("\n"), Some("text".to_string())))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!("🌐 Failed to fetch {}", url)))
                }
            }
            Err(_) => Ok(ChatOpResult::Message("🌐 curl command not available".to_string())),
        }
    }
}

/// SSL certificate information
pub struct SslCommand;

impl ChatCommand for SslCommand {
    fn name(&self) -> &'static str { "ssl" }
    fn description(&self) -> &'static str { "Show SSL certificate information" }
    fn usage(&self) -> &'static str { "/ssl <domain>" }
    fn aliases(&self) -> Vec<&'static str> { vec!["cert"] }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify a domain".to_string()));
        }
        
        let domain = &args[0];
        
        match Command::new("openssl")
            .args(&["s_client", "-connect", &format!("{}:443", domain), "-servername", domain])
            .stdin(std::process::Stdio::null())
            .output()
        {
            Ok(output) => {
                let result = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<String> = result
                    .lines()
                    .filter(|line| {
                        line.contains("subject=") || 
                        line.contains("issuer=") ||
                        line.contains("notBefore=") ||
                        line.contains("notAfter=") ||
                        line.contains("Verification:")
                    })
                    .take(8)
                    .map(|line| format!("🔒 {}", line.trim()))
                    .collect();
                
                if lines.is_empty() {
                    Ok(ChatOpResult::Message(format!("🔒 Could not retrieve SSL certificate for {}", domain)))
                } else {
                    Ok(ChatOpResult::Block(lines))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!("🔒 SSL check not available. Try: https://www.ssllabs.com/ssltest/analyze.html?d={}", domain))),
        }
    }
}

/// Tor exit node checker
pub struct TorCheckCommand;

impl ChatCommand for TorCheckCommand {
    fn name(&self) -> &'static str { "torcheck" }
    fn description(&self) -> &'static str { "Check if IP is a Tor exit node" }
    fn usage(&self) -> &'static str { "/torcheck <ip>" }
    
    fn execute(&self, args: Vec<String>, _context: &CommandContext) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments("Please specify an IP address".to_string()));
        }
        
        let ip = &args[0];
        
        // Basic IP validation
        if !ip.chars().all(|c| c.is_ascii_digit() || c == '.') || ip.split('.').count() != 4 {
            return Err(ChatOpError::InvalidSyntax("Please provide a valid IPv4 address".to_string()));
        }
        
        // Try checking with a Tor exit list service
        match Command::new("curl")
            .args(&["-s", "--max-time", "5", &format!("https://check.torproject.org/torbulkexitlist?ip={}", ip)])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    if result.contains(ip) {
                        Ok(ChatOpResult::Message(format!("🧅 {} is a Tor exit node", ip)))
                    } else {
                        Ok(ChatOpResult::Message(format!("🧅 {} is not a known Tor exit node", ip)))
                    }
                } else {
                    Ok(ChatOpResult::Message(format!("🧅 Could not check Tor status for {}", ip)))
                }
            }
            Err(_) => Ok(ChatOpResult::Message(format!("🧅 Tor check not available. Try: https://metrics.torproject.org/exonerator.html?ip={}", ip))),
        }
    }
}
