# BHCLI

![screenshot](strange_bhcli.jpg "strange_bhcli")

## Description

This is a CLI client for any of [le-chat-php](https://github.com/DanWin/le-chat-php)  
Officially supported chats are [Black Hat Chat](http://blkhatjxlrvc5aevqzz5t6kxldayog6jlx5h7glnu44euzongl4fh5ad.onion)

Tested working on [ --url ] :
- [PopPooB's Chat](http://vfdvqflzfgwnejh6rrzjnuxvbnpgjr4ursv4moombwyauot5c2z6ebid.onion/chat.php)

## Pre-built binaries

Pre-buit binaries can be found on the [official website](http://git.dkforestseeaaq2dqz2uflmlsybvnq2irzn4ygyvu53oazyorednviid.onion/Strange/bhcli/releases)

## Features

- Sound notifications when tagged/pmmed
- Private messages `/pm username message`
- Kick someone `/kick username message` | `/k username message` (Members +)
- Delete last message `/dl`
- Delete last X message `/dl5` will delete the last 5 messages
- Delete all messages `/dall`
- Ignore someone `/ignore username`
- Unignore someone `/unignore username`
- (Dasho) Ban a username and kick `/ban username` (Members +)
- (Dasho) Ban a username exactly `/ban "username"` (Members +)
- (Dasho) Filter messages containing text `/filter text` (Members +)
- (Dasho) List banned usernames `/banlist` (Members +)
- (Dasho) List exact banned usernames `/banexactlist` (Members +)
- (Dasho) List filtered message terms `/filterlist` (Members +)
- (Dasho) Unban a username `/unban username` (Members +)
- (Dasho) Remove a message filter `/unfilter text` (Members +)
- Toggle notifications sound `m`
- Toggle a "guest" view, by filtering out PMs and "Members chat" `shift+G`
- Toggle a "members" view, by filtering out PMs and "Guest chat" `shift+M`
- Filter messages `/f terms`
- Copy a selected message to clipboard `ctrl+C` | `y`
- Copy the first link in a message to clipboard `shift+Y`
- Directly tag author of selected message `t` will prefil the input with `@username `
- Directly private message author of selected message `p` will prefil the input with `/pm username `
- (Dasho) Shortcut to kick author of selected message `ctrl+k` will prefill with `/pm <master> #kick username` if a master account is set, otherwise `/kick username `
- (Dasho) Shortcut to ban author of selected message `ctrl+b` will prefill with `/pm <master> #ban username` if a master account is set, otherwise `/ban username ` (Again, only useful for members+ users)
- (Dasho) Use `ctrl+a` to prefill the input with `/pm <master> /m ` when a master account is set, or `/m ` when none is configured (Again, only useful for members+ users)
- captcha is displayed directly in terminal 10 times the real size
- Upload file `/u C:\path\to\file.png @username message` (@username is optional) `@members` for members group
- `<tab>` to autocomplete usernames while typing
- `ctrl + w` or !warn username to send a pre-kick warning message to a user
  [ Only for members+ users ]
  > This is your warning @username, will be kicked next !rules
- Can hide messages with `backspace`, hidden messages can be viewed by toggling
  `ctrl+  H`. 
  > - Hidden messages are just hidden from the view, they are not deleted
  > - Deleted messages once hidden can't be viewed again
- Download an embedded file into cwd with `d`
- Download an embedded file and open it with xdg-open into cwd with `D`
- `shift + T` for translating text to english. [ must have translate-shell installed on arch or debain ]
    > pacman -S translate-shell
- Custom personal command creation for members+ [ read Command Creation ]
- Set alternate and master accounts per profile using `/set alt <username>` and `/set master <username>`

### Editing mode
- `ctrl+A` Move cursor to start of line
- `ctrl+E` Move cursor to end of line
- `ctrl+F` Move cursor a word forward
- `ctrl+B` Move cursor a word backward

### Messages navigation
- Page down the messages list `ctrl+D` | `page down`
- Page up the messages list `ctrl+U` | `page up`
- Going down 1 message `j` | `down arrow`
- Going down 5 message `J(CAPS)`
- Going up 1 message `k` | `up arrow`
- Going up 5 message `K(CAPS)`
- Jump to Top Message `gg`

## Build from source

### Windows

- Install C++ build tools https://visualstudio.microsoft.com/visual-cpp-build-tools/
- Install Rust https://www.rust-lang.org/learn/get-started
- Download & extract code source
- Compile with `cargo build --release`

### OSx

- Install Rust https://www.rust-lang.org/learn/get-started
- Compile with `cargo build --release`

### Linux

- Install Rust
- Install dependencies `apt-get install -y pkg-config libasound2-dev libssl-dev cmake libfreetype6-dev libexpat1-dev libxcb-composite0-dev libx11-dev`
- The manual way
> - Compile with `cargo build --release`
> - Run with `./target/release/bhcli`
> - You can move the binary to `/opt` to make it available system wide [ given that u have /opt in $PATH ]
- The MAKEFILE way
> - Compile with `make linux`
> - Run with bhcli [ given that u have /opt in $PATH ]
- The bhcli.log file will be created in the same directory as the pwd you run
  the binary from

## Cross compile

`cargo build --release --target x86_64-pc-windows-gnu`


## Profiles

To automatically login when starting the application, you can put the following content in your config file `/path/to/rs.bhcli/default-config.toml`

```toml
[profiles]

[profiles.default]
username = "username"
password = "password"
alt_account = "myAlt" # Optional, only for members+ (Dasho)
master_account = "myMain" # Optional, only for members+ (Dasho)
```
## Custom Commands

U can create ur own custom personal commands using the format below.<br> 
The commands are not created on the server but rather edited on clien tand sen
tot server.<br> 
Comands must start from "!" in the textbox, but "!" are not required in config.

```toml
[commands]

command1 = "This is the mesage that will be posted"
hello = "hello everyone !"
```

## Configuration file

The configuration is stored using `confy`. On Linux this is usually
`~/.config/bhcli/bhcli.toml`. You can edit this file to preload profiles,
create custom commands and maintain filters.

For members+ users, you can set the `alt_account` and `master_account` fields in the config file. (Dasho)
To manually add or remove banned usernames or message filters you can edit the
`bad_usernames`, `bad_exact_usernames` and `bad_messages` arrays in this file:

```toml
bad_usernames = ["spammer1", "spammer2"]
bad_exact_usernames = ["baduser"]
bad_messages = ["buy now", "free money"]
```

Filters modified using `/ban`, `/ban "name"`, `/filter`, `/unban` and `/unfilter` are saved
back to this file automatically and any custom commands in the `[commands]`
section are preserved.

## Command Line Arguments

BHCLI supports various command-line arguments for configuration and customization:

### Authentication & Profile
- `-u, --username <USERNAME>` - Set username (can also use `BHC_USERNAME` env var)
- `-p, --password <PASSWORD>` - Set password (can also use `BHC_PASSWORD` env var)
- `-c, --profile <PROFILE>` - Select configuration profile (default: "default")
- `--session <SESSION>` - Use existing session ID to skip login

### Connection & Network
- `--url <URL>` - Override chat server URL
- `--page-php <PAGE>` - Override chat page filename (default: chat.php)
- `-s, --socks-proxy-url <URL>` - SOCKS proxy URL (default: socks5h://127.0.0.1:9050, can use `BHC_PROXY_URL` env var)
- `--no-proxy` - Disable proxy usage
- `--datetime-fmt <FORMAT>` - Override datetime format string
- `--members-tag <TAG>` - Override members tag format

### Display & Behavior
- `-g, --guest-color <COLOR>` - Set guest color theme
- `-m, --manual-captcha` - Enable manual captcha solving (can also use `BHC_MANUAL_CAPTCHA` env var)
- `-r, --refresh-rate <SECONDS>` - Message refresh rate in seconds (default: 5, can use `BHC_REFRESH_RATE` env var)
- `--max-login-retry <COUNT>` - Maximum login retry attempts (default: 5, can use `BHC_MAX_LOGIN_RETRY` env var)
- `--sxiv` - Enable sxiv image viewer integration

### Integrations
- `--dkf-api-key <KEY>` - DKF API key for notifications (can also use `DKF_API_KEY` env var)
- `--dnmx-username <USERNAME>` - DNMX email username (can also use `DNMX_USERNAME` env var)
- `--dnmx-password <PASSWORD>` - DNMX email password (can also use `DNMX_PASSWORD` env var)

### Advanced Options
- `-d, --dan` - Enable special DAN mode features
- `--keepalive-send-to <TARGET>` - Override keepalive message target (default: "0")

### Usage Examples

```bash
# Basic usage with username and password
bhcli -u myusername -p mypassword

# Use a specific profile
bhcli -c myprofile

# Connect through different proxy
bhcli -s socks5h://127.0.0.1:9150

# Disable proxy completely
bhcli --no-proxy

# Use custom refresh rate
bhcli -r 3

# Connect to different chat server
bhcli --url "http://example.onion" --page-php "chat.php"

# Enable manual captcha solving
bhcli -m

# Use environment variables
export BHC_USERNAME="myuser"
export BHC_PASSWORD="mypass"
export BHC_PROXY_URL="socks5h://127.0.0.1:9150"
bhcli
```

Most settings can be configured via environment variables or saved in the configuration file for persistent use across sessions.

## Changelog

### Recent Updates (August 2025)

#### AI Moderation System
- **Added AI-powered moderation** with OpenAI integration for automated content filtering
- **Guest-only moderation**: AI moderation only applies to guests, members/staff/admins are exempt
- **Multi-layered protection**: Quick pattern matching + AI analysis for comprehensive coverage
- **AI conversation modes**:
  - `/ai off` - Completely disable AI
  - `/ai mod` - Enable moderation only  
  - `/ai reply all` - Enable replies to all messages + moderation
  - `/ai reply ping` - Enable replies only when tagged + moderation
- **Moderation strictness levels**: `/ai strict`, `/ai balanced`, `/ai lenient`
- **Enhanced pattern detection**: Comprehensive quick patterns for immediate filtering of inappropriate content
- **AI testing commands**: `/check ai` for system status, `/check mod <message>` to test moderation

#### Moderation Logging System
- **Added `/modlog on/off`** - Toggle moderation logging to admin channel (@0)
- **Detailed mod logs**: Track all moderation decisions, pattern matches, and AI analysis
- **Configurable logging**: Per-profile mod log settings saved to config

#### Message Threading & Performance
- **Per-message threading**: Each message now sends in its own thread to eliminate race conditions
- **Concurrent message processing**: User messages and system messages (AI, moderation) no longer block each other
- **Non-blocking channel operations**: Prevents deadlocks and improves responsiveness
- **Better concurrent handling**: Multiple messages can be sent simultaneously without interference

#### Enhanced Content Filtering
- **Expanded quick patterns**: Added comprehensive detection for inappropriate content involving minors
- **Improved spam detection**: Better recognition of repetitive/spam content
- **Allowlist bypass**: Allowlisted users bypass all content filters
- **Separated AI functions**: AI moderation and conversational AI now use different prompts for better accuracy

#### Technical Improvements
- **Thread-safe message sending**: All message operations now use dedicated threads
- **Improved error handling**: Better channel error management with try_send patterns
- **Enhanced logging**: More detailed moderation and system logs
- **Configuration persistence**: AI settings and mod log preferences saved per profile

These updates significantly improve the chat moderation capabilities while maintaining performance and preventing race conditions in message handling.
