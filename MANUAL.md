# BHCLI Manual

Complete reference guide for BHCLI. Everything you need to master the client.

## Table of Contents

1. [Command Line Arguments](#command-line-arguments)
2. [Configuration Files](#configuration-files)
3. [ChatOps Commands](#chatops-commands)
4. [Keyboard Shortcuts](#keyboard-shortcuts)
5. [Bot System](#bot-system)
6. [AI Features](#ai-features)
7. [Moderation System](#moderation-system)
8. [Custom Commands](#custom-commands)
9. [Advanced Features](#advanced-features)

---

## Command Line Arguments

BHCLI supports extensive command line configuration. Most settings can also be configured via environment variables or the config file.

### Authentication and Profile

```bash
-u, --username <USERNAME>
    Set your username
    Environment: BHC_USERNAME

-p, --password <PASSWORD>
    Set your password
    Environment: BHC_PASSWORD

-c, --profile <PROFILE>
    Select configuration profile (default: "default")
    Profiles are stored in ~/.config/bhcli/bhcli.toml

--session <SESSION>
    Use an existing session ID to skip login
    Useful for reconnecting without re-authenticating
```

### Connection and Network

```bash
--url <URL>
    Override the default chat server URL
    Example: --url "http://example.onion"

--page-php <PAGE>
    Override chat page filename (default: chat.php)
    Some servers use different page names

-s, --socks-proxy-url <URL>
    SOCKS proxy URL for Tor connection
    Default: socks5h://127.0.0.1:9050
    Environment: BHC_PROXY_URL

--no-proxy
    Disable proxy usage entirely
    Warning: Not recommended for .onion addresses

-r, --refresh-rate <SECONDS>
    Message refresh rate in seconds (default: 5)
    Lower = more responsive, higher = less bandwidth
    Environment: BHC_REFRESH_RATE

--datetime-fmt <FORMAT>
    Override datetime format string
    Uses standard strftime format

--members-tag <TAG>
    Override members tag format
    For custom chat server configurations
```

### Display and Behavior

```bash
-g, --guest-color <COLOR>
    Set guest color theme
    Accepts color names or hex codes

-m, --manual-captcha
    Enable manual captcha solving mode
    Shows captcha and waits for your input
    Environment: BHC_MANUAL_CAPTCHA

--sxiv
    Enable sxiv image viewer integration
    Opens downloaded images in sxiv automatically
```

### Bot System

```bash
--bot <NAME>
    Enable background bot with specified name
    Bot uses same credentials as main client
    Example: --bot Assistant

--bot-admins <USER1,USER2>
    Comma-separated list of bot administrators
    Only these users can issue admin commands
    Example: --bot-admins alice,bob,charlie

--bot-data-dir <PATH>
    Custom directory for bot data storage
    Default: bot_data/{botname}
    Example: --bot-data-dir /var/bhcli/bots
```

### External Integrations

```bash
--dkf-api-key <KEY>
    DKF API key for notifications
    Enables DKF chat notifications
    Environment: DKF_API_KEY

--dnmx-username <USERNAME>
    DNMX email username for mail notifications
    Environment: DNMX_USERNAME

--dnmx-password <PASSWORD>
    DNMX email password
    Environment: DNMX_PASSWORD
```

### Advanced Options

```bash
-d, --dan
    Enable special DAN mode features
    Internal use, not generally needed

--keepalive-send-to <TARGET>
    Override keepalive message target (default: "0")
    Used to prevent timeout on some servers
```

### Usage Examples

```bash
# Basic usage with credentials
bhcli -u myusername -p mypassword

# Use a specific profile
bhcli -c myprofile

# Connect through custom Tor port
bhcli -s socks5h://127.0.0.1:9150

# Direct connection (clearnet only!)
bhcli --no-proxy --url "http://clearnet-chat.com"

# Fast refresh rate for active monitoring
bhcli -r 2

# Run with bot and specific admins
bhcli --bot Assistant --bot-admins alice,bob

# Manual captcha solving
bhcli -m

# Using environment variables
export BHC_USERNAME="myuser"
export BHC_PASSWORD="mypass"
export BHC_PROXY_URL="socks5h://127.0.0.1:9050"
export OPENAI_API_KEY="sk-..."
bhcli
```

---

## Configuration Files

### Config File Location

BHCLI uses `confy` for configuration management. The config file is automatically created on first run.

**Linux/macOS:** `~/.config/bhcli/bhcli.toml`
**Windows:** `C:\Users\<username>\AppData\Roaming\bhcli\config\bhcli.toml`

### Profile Configuration

Store multiple profiles for different accounts or servers:

```toml
[profiles]

[profiles.default]
username = "yourusername"
password = "yourpassword"
# Optional: set master/alt accounts (members+ only)
alt_account = "YourAlt"
master_account = "YourMain"

[profiles.work]
username = "workaccount"
password = "differentpassword"

[profiles.bhc]
username = "bhcuser"
password = "bhcpass"

[profiles.poppooeb]
username = "pbuser"
password = "pbpass"
```

Switch profiles with `bhcli -c profile_name`

### Custom Commands

Create personal command shortcuts that work like aliases:

```toml
[commands]

# Simple text replacements
hello = "hey everyone, how's it going?"
afk = "stepping away, ping me if needed"
brb = "be right back"

# Common responses
rules = "1. Be respectful 2. No spam 3. Stay anonymous 4. Have fun"
help = "Need help? Check the docs or ask the community"

# Role-specific (for members/mods)
warn = "This is your warning @{}, next offense will be a kick"
welcome = "Welcome to the chat! Read !rules to get started"

# Technical
status = "System status: All services operational"
links = "Resources: GitHub: ... | Docs: ... | Mirror: ..."
```

Use custom commands by prefixing with `!`:
```
!hello
!rules
!warn username
```

### Filters and Moderation

Configure automatic filters (members+ feature):

```toml
# Ban users (fuzzy match)
bad_usernames = ["spammer1", "troll2", "badactor"]

# Ban exact usernames only
bad_exact_usernames = ["ExactSpammer", "Exact_Troll"]

# Filter messages containing these terms
bad_messages = ["spam text", "buy now", "click here"]

# Allowlist (bypass all filters)
allowlist = ["trusteduser1", "admin2"]
```

These can also be managed via commands:
- `/ban username` adds to bad_usernames
- `/ban "exact"` adds to bad_exact_usernames
- `/filter text` adds to bad_messages
- `/unban` and `/unfilter` remove entries

---

## ChatOps Commands

BHCLI includes 30+ developer-focused slash commands. All commands are available in-chat by typing `/command`.

### Documentation and Lookup

**`/help [command]`**
Show all available ChatOps commands or detailed help for a specific command.

```
/help
/help github
/help hash
```

**`/man <command>`**
Display system manual pages for commands.

```
/man curl
/man grep
/man ssh
```

**`/doc <language> <term>`**
Language-specific documentation lookup.

Supported languages: rust, python, javascript, go, java, cpp, ruby, php

```
/doc rust HashMap
/doc python dict
/doc javascript Promise
```

**`/explain <concept>`**
AI-powered explanation of programming concepts (requires OPENAI_API_KEY).

```
/explain recursion
/explain async/await
/explain blockchain
```

**`/cheat <tool>`**
Quick reference cheatsheets for common tools.

```
/cheat vim
/cheat git
/cheat tmux
```

**`/stackoverflow <query>`**
Search Stack Overflow for programming questions.

```
/stackoverflow rust error handling
/stackoverflow python list comprehension
```

**`/ref <language>`**
Quick access to language reference documentation.

```
/ref rust
/ref python
/ref javascript
```

### Tooling and Utilities

**`/hash <algorithm> <text>`**
Generate cryptographic hashes.

Algorithms: md5, sha1, sha256, sha512

```
/hash sha256 hello world
/hash md5 test data
/hash sha512 secret message
```

**`/uuid`**
Generate a random UUID v4.

```
/uuid
```

**`/base64 <encode|decode> <text>`**
Base64 encoding and decoding.

```
/base64 encode "hello world"
/base64 decode "aGVsbG8gd29ybGQ="
```

**`/regex <pattern> <text>`**
Test regular expressions and see matches.

```
/regex "\d+" "abc 123 def 456"
/regex "^[a-z]+" "hello123world"
```

**`/whois <domain>`**
Domain WHOIS lookup for registration information.

```
/whois github.com
/whois example.org
```

**`/dig <domain>`**
DNS record lookup and resolution.

```
/dig google.com
/dig example.com A
/dig github.com MX
```

**`/ipinfo <ip>`**
Get detailed information about IP addresses.

```
/ipinfo 8.8.8.8
/ipinfo 1.1.1.1
```

**`/rand <min> <max>`**
Generate random numbers within range.

```
/rand 1 100
/rand 0 1
/rand 100 1000
```

**`/time`**
Display current timestamp and timezone information.

```
/time
```

### Chat Features

**`/chatlink <message_id>`**
Create shareable links to specific chat messages.

```
/chatlink 12345
```

**`/quote <user> <message>`**
Quote and reference messages from other users.

```
/quote alice "that's a great idea"
/quote bob "I disagree with that approach"
```

**`/rooms`**
List all available chat rooms and their status.

```
/rooms
```

**`/whereis <user>`**
Find which rooms a user is currently in.

```
/whereis alice
/whereis bob
```

### AI Integration

All AI commands require `OPENAI_API_KEY` environment variable.

**`/summarize <text>`**
AI-powered text summarization.

```
/summarize "long text here..."
```

**`/translate <language> <text>`**
Translate text to specified language.

Supports 100+ languages.

```
/translate spanish "Hello, how are you?"
/translate french "Good morning"
/translate german "Thank you very much"
```

**`/detect <text>`**
Detect language of text with confidence score.

```
/detect "Bonjour, comment allez-vous?"
```

**`/sentiment <text>`**
Analyze sentiment of text (positive/negative/neutral).

```
/sentiment "This is amazing!"
/sentiment "I'm not sure about this"
```

**`/atmosphere`**
Analyze current chat atmosphere and mood.

```
/atmosphere
```

**`/modcheck <message>`**
Test moderation system on a message.

```
/modcheck "potentially problematic content"
```

**`/fix <code>`**
Get AI suggestions for fixing code issues.

```
/fix "fn main() { println!(x) }"
```

**`/review <code>`**
AI-powered code quality review and suggestions.

```
/review "def factorial(n): return n * factorial(n-1)"
```

### GitHub and Package Management

**`/github <user/repo> [subcommand]`** (alias: `/gh`)
Get repository information and links.

```
/github rust-lang/rust              # Basic repo info
/github microsoft/vscode issues     # Link to issues
/github torvalds/linux latest       # Latest release
/github user/repo file src/main.rs  # Link to specific file
```

**`/gist <code>`**
Create GitHub Gists (requires GitHub CLI authentication).

```
/gist "console.log('hello');"
```

**`/crates <crate_name>`**
Rust crate information from crates.io.

```
/crates serde
/crates tokio
/crates reqwest
```

**`/npm <package_name>`**
NPM package information and installation commands.

```
/npm express
/npm react
/npm axios
```

**`/pip <package_name>`** (alias: `/pypi`)
Python package info from PyPI.

```
/pip requests
/pip django
/pip numpy
```

### Network Diagnostics

**`/ping <host>`**
Test network connectivity and response times.

```
/ping google.com
/ping 8.8.8.8
```

**`/traceroute <host>`**
Trace network path to destination.

```
/traceroute google.com
```

**`/nslookup <domain>`**
DNS name resolution and record lookup.

```
/nslookup github.com
```

**`/netstat`**
Display active network connections and listening ports.

```
/netstat
```

**`/portscan <host> <port_range>`**
Scan ports on a host (use responsibly).

```
/portscan localhost 80-100
```

**`/headers <url>`**
Show HTTP headers for a URL.

```
/headers https://example.com
```

**`/curl <url>`**
Fetch URL content via curl.

```
/curl https://api.github.com
```

**`/ssl <domain>`**
Check SSL certificate information.

```
/ssl github.com
/ssl google.com
```

**`/torcheck`**
Check if you're connected through Tor.

```
/torcheck
```

### Miscellaneous

**`/ascii <text>`**
Convert text to ASCII art.

```
/ascii BHCLI
/ascii Hello
```

**`/fortune`**
Display random fortune/quote.

```
/fortune
```

**`/motd`**
Show message of the day.

```
/motd
```

**`/afk [message]`**
Set away from keyboard status.

```
/afk
/afk "lunch break"
```

**`/alias <name> <command>`**
Create personal command aliases.

```
/alias list                    # Show all aliases
/alias gh "/github"           # Create alias
/alias remove gh              # Remove alias
```

**`/version`**
Display BHCLI version and system information.

```
/version
```

### Command Permissions

Commands respect user role hierarchy:

- **Guest**: Documentation, basic tools, read-only commands
- **Member**: Full access to all ChatOps commands
- **Staff/Admin**: Complete access plus future admin commands

Some commands (like `/gist`) require external authentication (GitHub CLI).

---

## Keyboard Shortcuts

BHCLI supports extensive keyboard navigation and shortcuts.

### Message Navigation

```
j               Move down one message
k               Move up one message
J               Jump down 5 messages
K               Jump up 5 messages
gg              Jump to top message
G               Jump to bottom message (latest)

Down arrow      Move down one message
Up arrow        Move up one message
Page Down       Scroll down one page
Page Up         Scroll up one page
ctrl+d          Scroll down one page
ctrl+u          Scroll up one page
```

### Quick Actions

```
t               Tag author of selected message (@username)
p               PM author of selected message (/pm username)
y               Copy selected message to clipboard
Y (shift+y)     Copy first link in message
d               Download embedded file
D (shift+d)     Download and open file with xdg-open

Enter           Start typing in input box
Escape          Exit input mode back to navigation
```

### View Toggles

```
m               Toggle sound notifications (mute/unmute)
G (shift+g)     Toggle guest view (hide members/PM)
M (shift+m)     Toggle members view (hide guests/PM)
ctrl+h          Toggle hidden messages view
Backspace       Hide selected message
```

### Input Editing (When focused on input box)

```
ctrl+a          Move cursor to start of line
ctrl+e          Move cursor to end of line
ctrl+f          Move cursor forward one word
ctrl+b          Move cursor backward one word
ctrl+l          Toggle multiline input mode

ctrl+.          Open external editor (nvim/vim/nano)
ctrl+x          Open external editor (alternative)
ctrl+o          Open external editor (alternative)

Up arrow        Navigate command history (previous)
Down arrow      Navigate command history (next)
Tab             Autocomplete username

ctrl+w          Delete word backward
ctrl+u          Delete from cursor to start of line
ctrl+k          Delete from cursor to end of line
```

### Multiline Input Mode

When multiline mode is enabled (`ctrl+l`):

```
Enter           Insert newline (not send)
ctrl+Enter      Send message
ctrl+l          Toggle back to single-line mode
Escape          Exit to normal mode
Up/Down         Navigate command history
```

### Moderation Shortcuts (Members+ only)

```
ctrl+k          Prefill kick command for selected message
                If master account set: /pm <master> #kick username
                Otherwise: /kick username

ctrl+b          Prefill ban command for selected message
                If master account set: /pm <master> #ban username
                Otherwise: /ban username

ctrl+a          Prefill members group message
                If master account set: /pm <master> /m
                Otherwise: /m

ctrl+w          Send warning message (!warn username)
```

---

## Bot System

BHCLI includes a powerful background bot system that runs alongside your main client.

### Starting a Bot

```bash
# Basic bot
./bhcli --bot BotName

# Bot with admin users
./bhcli --bot Assistant --bot-admins alice,bob,charlie

# Custom data directory
./bhcli --bot Assistant --bot-data-dir /var/bhcli/bots
```

The bot uses the same credentials as your main client. It connects independently and maintains its own session.

### Bot Commands

Interact with the bot by mentioning it in chat. Only designated admins can use most commands.

**`@BotName help`**
List all available bot commands.

**`@BotName stats <username>`**
View detailed statistics for a user.

Shows:
- Total messages sent
- First/last seen timestamps
- Activity patterns by hour
- Average message length
- Most active hours
- Interaction patterns

```
@Assistant stats alice
```

**`@BotName recall <time>`**
Find messages from a specific time.

Time formats:
- HH:MM (today)
- HH:MM:SS (today)
- "1 hour ago"
- "2 days ago"

```
@Assistant recall 14:30
@Assistant recall 09:15:30
@Assistant recall "2 hours ago"
```

**`@BotName search <query>`**
Search message history for content.

Supports:
- Simple text search
- Case-insensitive matching
- Multiple word queries

```
@Assistant search "rust error"
@Assistant search bitcoin
```

**`@BotName export <username> <days>`**
Export a user's messages to JSON/CSV.

```
@Assistant export alice 7
@Assistant export bob 30
@Assistant export charlie 1
```

Files are saved to `bot_data/{botname}/exports/`

**`@BotName restore <message_id>`**
Restore a deleted message from bot memory.

```
@Assistant restore 12345
```

**`@BotName summary <hours>`**
Generate chat activity summary.

```
@Assistant summary 24
@Assistant summary 168   # Last week
```

**`@BotName users`**
List current online users.

```
@Assistant users
```

**`@BotName top <metric>`**
Show top users by various metrics.

Metrics:
- messages (message count)
- active (most active hours)
- words (word count)

```
@Assistant top messages
@Assistant top active
@Assistant top words
```

**`@BotName when <username>`**
Show when a user was last seen.

```
@Assistant when alice
```

**`@BotName active`**
Show currently active users and their activity levels.

```
@Assistant active
```

### Bot Data Storage

Bot data is stored in `bot_data/{botname}/`:

```
bot_data/
└── BotName/
    ├── message_history.json    # Complete chat history
    ├── user_stats.json         # User statistics database
    └── exports/                # Exported data files
        ├── alice_20250109.json
        └── bob_20250109.csv
```

All data is stored locally and never leaves your machine.

### Bot Features

**Perfect Memory**
Bots remember every message, even after restarts. Chat history persists indefinitely.

**User Analytics**
Comprehensive statistics including:
- Activity patterns by hour and day
- Message frequency and length
- Interaction patterns with other users
- First/last seen timestamps
- Online time estimation

**Message Recovery**
Recover deleted messages from bot memory. Useful for accident prevention and moderation.

**Data Export**
Generate research-ready archives:
- JSON format for programmatic access
- CSV format for spreadsheet analysis
- Filtered by user and time range

**AI Integration**
When `OPENAI_API_KEY` is set, bots gain:
- Intelligent chat summaries
- Sentiment analysis over time
- Pattern detection
- Enhanced moderation capabilities

### Bot Permissions

Most bot commands require admin status. Configure admins when starting:

```bash
./bhcli --bot Assistant --bot-admins alice,bob
```

Non-admin users can:
- View help (`@BotName help`)
- Check their own stats (`@BotName stats <self>`)

Admins can:
- All user commands
- Export data
- Restore messages
- View all statistics
- Generate summaries

---

## AI Features

BHCLI integrates OpenAI for advanced capabilities. Set `OPENAI_API_KEY` to enable AI features.

### AI Modes

Control how AI responds in chat:

```bash
/ai off          # Disable AI completely
/ai mod          # Moderation only (no replies)
/ai reply all    # Reply to all messages + moderation
/ai reply ping   # Reply only when mentioned + moderation
```

Current mode is saved to your profile.

### Moderation System

AI-powered content moderation for guest messages (members/staff/admins are exempt).

**Strictness Levels:**

```bash
/ai strict       # Aggressive filtering, low tolerance
/ai balanced     # Moderate filtering (default)
/ai lenient      # Relaxed filtering, high tolerance
```

**How it works:**
1. Quick pattern matching catches obvious violations
2. AI analyzes context and intent
3. Severity scored 0-10
4. Action taken based on strictness level

**Actions:**
- Warn: Log the issue, no action
- Kick: Remove user from chat
- Ban: Permanent ban from chat

**Testing moderation:**

```bash
/check ai                    # Check AI system status
/check mod "test message"    # Test moderation on sample text
```

### Moderation Logging

Enable logging of moderation decisions:

```bash
/modlog on       # Enable mod logs
/modlog off      # Disable mod logs
```

Logs are sent to admin channel (`@0`) and include:
- Trigger reason (pattern or AI)
- Severity score
- Action taken
- Original message
- Username

### Translation

Real-time translation across 100+ languages:

```bash
/translate spanish "Hello, how are you?"
/translate french "Good morning everyone"
/translate german "Thank you for your help"
```

**Auto-detection:**

```bash
/detect "Bonjour tout le monde"
# Output: Detected: French (confidence: 0.95)
```

Translate-shell fallback if OpenAI unavailable (requires `translate-shell` package).

### Sentiment Analysis

Analyze emotional tone of messages:

```bash
/sentiment "This is amazing, I love it!"
# Output: Positive (confidence: 0.89)

/sentiment "I'm not sure about this approach"
# Output: Neutral (confidence: 0.72)
```

**Chat atmosphere:**

```bash
/atmosphere
# Analyzes recent messages and provides mood summary
```

### Code Assistance

**Fix broken code:**

```bash
/fix "fn main() { println!(x) }"
# Suggests: Add missing variable declaration
```

**Code review:**

```bash
/review "def factorial(n): return n * factorial(n-1)"
# Provides: Missing base case, will cause infinite recursion
```

**Explanation:**

```bash
/explain "What is a closure in Rust?"
# Detailed explanation of Rust closures
```

### AI Configuration

AI behavior is controlled via:
1. Command line: `/ai` commands
2. Environment: `OPENAI_API_KEY`
3. Config file: Settings persist per profile

**Privacy note:** AI features send messages to OpenAI API. Only enabled when explicitly configured.

---

## Moderation System

Comprehensive moderation tools for members, staff, and admins.

### Basic Moderation

**Kick user:**

```bash
/kick username reason
/k username reason      # Shortcut
```

Keyboard shortcut: `ctrl+k` on selected message

**Ban user (fuzzy match):**

```bash
/ban username
```

Keyboard shortcut: `ctrl+b` on selected message

**Ban exact username:**

```bash
/ban "exact username"
```

**Warning:**

```bash
!warn username
```

Keyboard shortcut: `ctrl+w`

Sends: "This is your warning @username, will be kicked next !rules"

### Filter Management

**Add message filter:**

```bash
/filter spam text here
```

Automatically kicks any guest who posts messages containing "spam text here".

**Remove filter:**

```bash
/unfilter spam text here
```

**List filters:**

```bash
/filterlist
```

### Ban Management

**Unban user:**

```bash
/unban username
```

**List banned usernames:**

```bash
/banlist
```

**List exact banned usernames:**

```bash
/banexactlist
```

### Message Deletion

**Delete last message:**

```bash
/dl
```

**Delete last N messages:**

```bash
/dl5     # Delete last 5
/dl10    # Delete last 10
```

**Delete all your messages:**

```bash
/dall
```

### Allowlist System

Add trusted users to allowlist (bypass all filters):

Edit `~/.config/bhcli/bhcli.toml`:

```toml
allowlist = ["trusteduser1", "admin2", "moderator3"]
```

Allowlisted users:
- Bypass pattern filters
- Bypass AI moderation
- Can never be auto-kicked

### Master/Alt Account System (Dasho-specific)

Link alt and master accounts for command delegation:

```bash
/set alt AltAccountName
/set master MasterAccountName
```

When configured:
- `ctrl+k` sends: `/pm <master> #kick username`
- `ctrl+b` sends: `/pm <master> #ban username`
- `ctrl+a` sends: `/pm <master> /m`

This allows alt accounts to delegate moderation to master account.

### Moderation Best Practices

1. **Warn first**: Give users a chance to correct behavior
2. **Document reasons**: Include reason in kick/ban commands
3. **Use filters**: Automate obvious spam/abuse
4. **Review logs**: Check `/modlog` output regularly
5. **Allowlist carefully**: Only trusted long-term users

---

## Custom Commands

Create personal shortcuts for frequently used messages.

### Creating Commands

Edit `~/.config/bhcli/bhcli.toml`:

```toml
[commands]

hello = "hey everyone, how's it going?"
afk = "stepping away for a bit, ping me if needed"
brb = "be right back in 5"
rules = "1. Be cool 2. No spam 3. Stay anonymous"
```

### Using Commands

Prefix with `!` in chat:

```
!hello
!afk
!rules
```

The command expands to its full text before sending.

### Advanced Examples

**Variables (manual substitution):**

```toml
warn = "This is your warning @{}, next offense will be kicked"
```

Usage: Edit the message after typing `!warn` to insert username.

**Long messages:**

```toml
welcome = """
Welcome to Black Hat Chat!

Rules:
1. Be respectful
2. No spam or flooding
3. Keep it legal
4. Stay anonymous

Need help? Type /help for commands.
"""
```

**Technical responses:**

```toml
rust_help = "Rust resources: https://doc.rust-lang.org | https://rust-lang.github.io/async-book/"
tor_setup = "Tor setup guide: https://... | Verify: https://check.torproject.org"
```

### Command Tips

1. Keep commands short and memorable
2. Use for frequently typed messages
3. Document complex commands
4. Update as needed, changes apply immediately
5. Share useful commands with the community

---

## Advanced Features

### External Editor Integration

Open nvim/vim/nano for composing long messages:

```
ctrl+.    or    ctrl+x    or    ctrl+o
```

How it works:
1. Launches your `$EDITOR` (or falls back to nvim → vim → nano → vi)
2. Loads current input content
3. After saving and exiting, automatically processes message
4. Supports all commands and prefixes (/pm, /m, /s, etc.)
5. Returns to chat immediately after sending

Perfect for:
- Long messages
- Code snippets
- Formatted text
- Multiple paragraphs

### Message History

Navigate previously sent messages:

```
Up arrow      Previous message
Down arrow    Next message
```

History filters by current input prefix:
- Typing `/pm` then up arrow shows only previous `/pm` commands
- Typing `!` then up arrow shows only custom commands

### File Operations

**Upload:**

```bash
/u /path/to/file.png @username message here
/u /path/to/file.png @members message
```

**Download:**

```
d          Download file in selected message
D          Download and open with xdg-open
```

Downloads go to current working directory.

### Hidden Messages

Hide individual messages without deleting:

```
Backspace       Hide selected message
ctrl+H          Toggle hidden messages view
```

Hidden messages:
- Still in database
- Only hidden from view
- Can be toggled visible
- Once deleted, cannot be recovered (unless bot has copy)

### Profile Management

Switch between saved profiles:

```bash
bhcli -c default
bhcli -c work
bhcli -c bhc
```

Each profile maintains:
- Credentials
- Master/alt configuration
- AI settings
- Mod log preferences
- Custom commands
- Filters and bans

### Session Persistence

Save your session to skip login next time:

After logging in, find your session ID in logs or config, then:

```bash
bhcli --session <SESSION_ID>
```

Useful for:
- Quick reconnects
- Avoiding captcha
- Maintaining state

Sessions expire after inactivity (server-dependent).

### Tor Configuration

**Default (recommended):**

```bash
bhcli -s socks5h://127.0.0.1:9050
```

**Tor Browser Bundle:**

```bash
bhcli -s socks5h://127.0.0.1:9150
```

**Custom Tor instance:**

```bash
bhcli -s socks5h://localhost:9999
```

**Disable Tor (clearnet only!):**

```bash
bhcli --no-proxy
```

Never use `--no-proxy` with .onion addresses.

### Refresh Rate Tuning

Balance responsiveness vs bandwidth:

```bash
bhcli -r 2     # Very responsive (2 second refresh)
bhcli -r 5     # Balanced (default)
bhcli -r 10    # Conservative bandwidth
```

Lower = more responsive but more bandwidth and server load.

### Logging

Logs are written to `bhcli.log` in current directory.

Contains:
- Connection events
- Errors and warnings
- Moderation decisions (if enabled)
- Debug information

Useful for troubleshooting.

---

## Troubleshooting

### Connection Issues

**Cannot connect:**
- Check Tor is running: `systemctl status tor`
- Verify proxy port: Usually 9050 for Tor, 9150 for Tor Browser
- Try different refresh rate: `bhcli -r 10`
- Check server status (may be down)

**Timeout errors:**
- Increase refresh rate: `bhcli -r 10`
- Check network connectivity
- Verify .onion address is correct

### Captcha Problems

**Cannot see captcha:**
- Try manual mode: `bhcli -m`
- Check terminal supports images
- Increase terminal size

**Captcha fails repeatedly:**
- Use manual entry mode
- Try different terminal emulator
- Check OCR library is installed

### Audio Issues

**No sound:**
- Check audio system is running
- Verify sound notifications enabled (press `m` to toggle)
- Build without audio if not needed: `make build-no-audio`

**Audio errors:**
- Build with `--no-default-features` to disable audio
- Check ALSA/PulseAudio configuration

### Performance Issues

**High CPU usage:**
- Check refresh rate (increase value)
- Disable AI features if not needed
- Kill background processes

**High memory:**
- Clear message history (restart client)
- Disable bot system if running
- Check for memory leaks (report if found)

### Configuration Issues

**Config not found:**
- Check path: `~/.config/bhcli/bhcli.toml`
- Create manually if missing
- Run once to generate default config

**Profile not working:**
- Verify profile name matches config
- Check TOML syntax is valid
- Look for errors in bhcli.log

---

## Tips and Tricks

### Power User Shortcuts

Combine features for maximum efficiency:

```bash
# Quick moderation workflow
# 1. Navigate to bad message (j/k)
# 2. Kick user (ctrl+k)
# 3. Edit reason, send
# 4. Add to ban list manually if needed

# Fast PM responses
# 1. Select message (j/k)
# 2. Start PM (p)
# 3. Type response
# 4. Send

# Efficient filtering
# 1. Add filter (/filter spam)
# 2. Save to config (automatic)
# 3. Applies to all future messages
```

### Workflow Optimization

**For moderators:**
1. Set master/alt accounts
2. Create custom commands for warnings
3. Enable mod logs
4. Set AI to "balanced" or "strict"
5. Maintain allowlist

**For developers:**
1. Use ChatOps for quick lookups
2. Keep external editor ready
3. Use multiline mode for code
4. Save code snippets as custom commands

**For power users:**
1. Multiple profiles for different identities
2. Custom commands for frequent responses
3. Hotkeys memorized for speed
4. Bot running for history/search

### Privacy and Security

**Best practices:**
1. Always use Tor for darknet chats
2. Different passwords per profile
3. Never share session IDs
4. Clear logs periodically
5. Use temporary directories for file downloads

**Operational security:**
1. No personally identifiable information
2. Different usernames across chats
3. Mind your timezone in timestamps
4. Be aware of writing style analysis
5. Use VPN + Tor for extra layers

### Community Guidelines

**Be a good citizen:**
1. Don't spam or flood
2. Respect moderators
3. Follow chat rules
4. Help new users
5. Report actual problems, not noise

**Technical etiquette:**
1. Use /pm for long conversations
2. Keep code snippets readable
3. Use /m for member discussions
4. Test commands before spamming chat
5. Read existing docs before asking

---

## Getting Help

**In order of preference:**

1. This manual (`MANUAL.md`)
2. Man page (`man bhcli`)
3. In-chat `/help` command
4. Community in chat
5. GitHub issues
6. Official mirror

**Before asking:**
- Check logs (`bhcli.log`)
- Verify configuration
- Try restarting client
- Test with default settings
- Read error messages carefully

Most issues are configuration-related. Check your config file, environment variables, and command line arguments.

---

Built for anonymity, optimized for speed, designed for developers.

Stay safe, stay anonymous, stay paranoid.
