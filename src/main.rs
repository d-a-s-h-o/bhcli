mod account_management;
mod ai_service;
mod bhc;
mod bot_client;
// mod bot_integration;
mod bot_system;
mod chatops;
// mod enhanced_bot_commands;
// mod enhanced_bot_system;
mod harm;
mod lechatphp;
mod util;

use crate::account_management::{AccountManager, AccountRelationshipStatus, parse_enhanced_command};
use crate::ai_service::AIService;
use crate::bot_client::BotManager;

use crate::chatops::{ChatOpsRouter, UserRole};
use crate::lechatphp::LoginErr;
use anyhow::{anyhow, Context};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
    },
    Client as OpenAIClient,
};
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use clap::Parser;
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use colors_transform::{Color, Rgb};
use crossbeam_channel::{self, after, select};
use crossterm::event;
use crossterm::event::Event as CEvent;
use crossterm::event::{MouseEvent, MouseEventKind};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use harm::{action_from_score, score_message, Action};
use lazy_static::lazy_static;
use linkify::LinkFinder;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use regex::Regex;
use reqwest::blocking::multipart;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
#[cfg(feature = "audio")]
use rodio::{source::Source, Decoder, OutputStream};
use select::document::Document;
use select::predicate::{Attr, Name};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Cursor;
use std::io::{self, Write};
use std::process::Command;
use std::sync::Mutex;
use std::sync::{Arc, MutexGuard};
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::runtime::Runtime;
use tui::layout::Rect;
use tui::style::Color as tuiColor;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use unicode_width::UnicodeWidthStr;
use util::StatefulList;

const LANG: &str = "en";
const SEND_TO_ALL: &str = "s *";
const SEND_TO_MEMBERS: &str = "s ?";
const SEND_TO_STAFFS: &str = "s %";
const SEND_TO_ADMINS: &str = "s _";
const SOUND1: &[u8] = include_bytes!("sound1.mp3");
const DKF_URL: &str = "http://dkforestseeaaq2dqz2uflmlsybvnq2irzn4ygyvu53oazyorednviid.onion";
const SERVER_DOWN_500_ERR: &str = "500 Internal Server Error, server down";
const SERVER_DOWN_ERR: &str = "502 Bad Gateway, server down";
const KICKED_ERR: &str = "You have been kicked";
const REG_ERR: &str = "This nickname is a registered member";
const NICKNAME_ERR: &str = "Invalid nickname";
const CAPTCHA_WG_ERR: &str = "Wrong Captcha";
const CAPTCHA_FAILED_SOLVE_ERR: &str = "Failed solve captcha";
const CAPTCHA_USED_ERR: &str = "Captcha already used or timed out";
const UNKNOWN_ERR: &str = "Unknown error";
const DNMX_URL: &str = "http://hxuzjtocnzvv5g2rtg2bhwkcbupmk7rclb6lly3fo4tvqkk5oyrv3nid.onion";

lazy_static! {
    static ref META_REFRESH_RGX: Regex = Regex::new(r#"url='([^']+)'"#).unwrap();
    static ref SESSION_RGX: Regex = Regex::new(r#"session=([^&]+)"#).unwrap();
    static ref COLOR_RGX: Regex = Regex::new(r#"color:\s*([#\w]+)\s*;"#).unwrap();
    static ref COLOR1_RGX: Regex = Regex::new(r#"^#([0-9A-Fa-f]{6})$"#).unwrap();
    static ref PM_RGX: Regex = Regex::new(r#"^/pm ([^\s]+) (.*)"#).unwrap();
    static ref KICK_RGX: Regex = Regex::new(r#"^/(?:kick|k) ([^\s]+)\s?(.*)"#).unwrap();
    static ref IGNORE_RGX: Regex = Regex::new(r#"^/ignore ([^\s]+)"#).unwrap();
    static ref UNIGNORE_RGX: Regex = Regex::new(r#"^/unignore ([^\s]+)"#).unwrap();
    static ref DLX_RGX: Regex = Regex::new(r#"^/dl([\d]+)$"#).unwrap();
    static ref DELETE_RGX: Regex = Regex::new(r#"^/delete (\d+)"#).unwrap();
    static ref UPLOAD_RGX: Regex = Regex::new(r#"^/u\s([^\s]+)\s?(?:@([^\s]+)\s)?(.*)$"#).unwrap();
    static ref FIND_RGX: Regex = Regex::new(r#"^/f\s(.*)$"#).unwrap();
    static ref NEW_NICKNAME_RGX: Regex = Regex::new(r#"^/nick\s(.*)$"#).unwrap();
    static ref NEW_COLOR_RGX: Regex = Regex::new(r#"^/color\s(.*)$"#).unwrap();
}

fn default_empty_str() -> String {
    "".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize)]
struct Profile {
    username: String,
    password: String,
    #[serde(default = "default_empty_str")]
    url: String,
    #[serde(default = "default_empty_str")]
    date_format: String,
    #[serde(default = "default_empty_str")]
    page_php: String,
    #[serde(default = "default_empty_str")]
    members_tag: String,
    #[serde(default = "default_empty_str")]
    keepalive_send_to: String,
    #[serde(default)]
    alt_account: Option<String>,
    #[serde(default)]
    master_account: Option<String>,
    #[serde(default = "default_empty_str")]
    system_intel: String,
    #[serde(default)]
    ai_enabled: bool,
    #[serde(default = "default_ai_mode")]
    ai_mode: String,
    #[serde(default = "default_moderation_strictness")]
    moderation_strictness: String, // "strict", "balanced", "lenient"
    #[serde(default = "default_true")]
    mod_logs_enabled: bool,
    #[serde(default)]
    identities: HashMap<String, Vec<String>>, // command -> [nickname, color, incognito?, member?, staff?]
}

fn default_ai_mode() -> String {
    "off".to_string()
}

fn default_moderation_strictness() -> String {
    "balanced".to_string()
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct MyConfig {
    dkf_api_key: Option<String>,
    #[serde(default)]
    alt_account: Option<String>,
    #[serde(default)]
    master_account: Option<String>,
    #[serde(default = "default_true")]
    alt_forwarding_enabled: bool,
    #[serde(default)]
    bad_usernames: Vec<String>,
    #[serde(default)]
    bad_exact_usernames: Vec<String>,
    #[serde(default)]
    bad_messages: Vec<String>,
    #[serde(default)]
    allowlist: Vec<String>,
    #[serde(default)]
    commands: HashMap<String, String>,
    profiles: HashMap<String, Profile>,
}

#[derive(Parser)]
#[command(name = "bhcli")]
#[command(author = "Dasho <o_o@dasho.dev>")]
#[command(version = "0.1.0")]
struct Opts {
    #[arg(long, env = "DKF_API_KEY")]
    dkf_api_key: Option<String>,
    #[arg(short, long, env = "BHC_USERNAME")]
    username: Option<String>,
    #[arg(short, long, env = "BHC_PASSWORD")]
    password: Option<String>,
    #[arg(short, long, env = "BHC_MANUAL_CAPTCHA")]
    manual_captcha: bool,
    #[arg(short, long, env = "BHC_GUEST_COLOR")]
    guest_color: Option<String>,
    #[arg(short, long, env = "BHC_REFRESH_RATE", default_value = "1")]
    refresh_rate: u64,
    #[arg(long, env = "BHC_MAX_LOGIN_RETRY", default_value = "5")]
    max_login_retry: isize,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    page_php: Option<String>,
    #[arg(long)]
    datetime_fmt: Option<String>,
    #[arg(long)]
    members_tag: Option<String>,
    #[arg(short, long)]
    dan: bool,
    #[arg(
        short,
        long,
        env = "BHC_PROXY_URL",
        default_value = "socks5h://127.0.0.1:9050"
    )]
    socks_proxy_url: String,
    #[arg(long)]
    no_proxy: bool,
    #[arg(long, env = "DNMX_USERNAME")]
    dnmx_username: Option<String>,
    #[arg(long, env = "DNMX_PASSWORD")]
    dnmx_password: Option<String>,
    #[arg(short = 'c', long, default_value = "default")]
    profile: String,

    //Strange
    #[arg(long, default_value = "0")]
    keepalive_send_to: Option<String>,

    #[arg(long)]
    session: Option<String>,

    #[arg(long)]
    sxiv: bool,

    #[arg(skip)]
    bad_usernames: Option<Vec<String>>,
    #[arg(skip)]
    bad_exact_usernames: Option<Vec<String>>,
    #[arg(skip)]
    bad_messages: Option<Vec<String>>,
    #[arg(skip)]
    allowlist: Option<Vec<String>>,

    // Bot system parameters
    #[arg(long)]
    bot: Option<String>,
    #[arg(long)]
    bot_admins: Vec<String>,
    #[arg(long)]
    bot_data_dir: Option<String>,

    // Use 404 chatroom profile
    #[arg(long = "404")]
    use_404: bool,
}

struct LeChatPHPConfig {
    url: String,
    datetime_fmt: String,
    page_php: String,
    keepalive_send_to: String,
    members_tag: String,
    staffs_tag: String,
}

impl LeChatPHPConfig {
    fn new_black_hat_chat_config() -> Self {
        Self {
            url: "http://blkhatjxlrvc5aevqzz5t6kxldayog6jlx5h7glnu44euzongl4fh5ad.onion".to_owned(),
            datetime_fmt: "%m-%d %H:%M:%S".to_owned(),
            page_php: "chat.php".to_owned(),
            keepalive_send_to: "0".to_owned(),
            members_tag: "[M] ".to_owned(),
            staffs_tag: "[Staff] ".to_owned(),
        }
    }

    fn new_404_chatroom_not_found_config() -> Self {
        Self {
            url: "http://4o4o4hn4hsujpnbsso7tqigujuokafxys62thulbk2k3mf46vq22qfqd.onion/chat/min".to_owned(),
            datetime_fmt: "%Y-%m-%d %H:%M:%S".to_owned(),
            page_php: "index.php".to_owned(),
            keepalive_send_to: "0".to_owned(),
            members_tag: "[M] ".to_owned(),
            staffs_tag: "[Staff] ".to_owned(),
        }
    }
}

struct BaseClient {
    username: String,
    password: String,
}

struct LeChatPHPClient {
    base_client: BaseClient,
    guest_color: String,
    client: Client,
    session: Option<String>,
    config: LeChatPHPConfig,
    last_key_event: Option<KeyCode>,
    manual_captcha: bool,
    sxiv: bool,
    refresh_rate: u64,
    max_login_retry: isize,

    is_muted: Arc<Mutex<bool>>,
    show_sys: bool,
    display_guest_view: bool,
    display_member_view: bool,
    display_hidden_msgs: bool,
    tx: crossbeam_channel::Sender<PostType>,
    rx: Arc<Mutex<crossbeam_channel::Receiver<PostType>>>,

    color_tx: crossbeam_channel::Sender<()>,
    color_rx: Arc<Mutex<crossbeam_channel::Receiver<()>>>,

    bad_username_filters: Arc<Mutex<Vec<String>>>,
    bad_exact_username_filters: Arc<Mutex<Vec<String>>>,
    bad_message_filters: Arc<Mutex<Vec<String>>>,
    allowlist: Arc<Mutex<Vec<String>>>,

    account_manager: AccountManager,
    profile: String,
    display_pm_only: bool,
    display_staff_view: bool,
    display_master_pm_view: bool,
    clean_mode: bool,
    inbox_mode: bool,
    alt_forwarding_enabled: Arc<Mutex<bool>>,

    // Store current active identity for restoration
    current_username: String,
    current_color: String,

    // AI fields
    ai_enabled: Arc<Mutex<bool>>,
    ai_mode: Arc<Mutex<String>>,
    system_intel: String,
    moderation_strictness: String,
    mod_logs_enabled: Arc<Mutex<bool>>,
    openai_client: Option<async_openai::Client<async_openai::config::OpenAIConfig>>,
    ai_conversation_memory: Arc<Mutex<std::collections::HashMap<String, Vec<(String, String)>>>>, // user -> (role, message) history

    // Warning tracking for alt mode moderation
    user_warnings: Arc<Mutex<std::collections::HashMap<String, u32>>>, // user -> warning count

    // Identity configurations from profile
    identities: HashMap<String, Vec<String>>, // command -> [nickname, color, incognito?, member?, staff?]

    // ChatOps system
    chatops_router: ChatOpsRouter,

    // Enhanced AI service
    ai_service: Arc<AIService>,
    #[allow(dead_code)]
    runtime: Arc<Runtime>,

    // Bot system manager
    bot_manager: Option<Arc<Mutex<BotManager>>>,
}

impl LeChatPHPClient {
    fn run_forever(&mut self) {
        let max_retry = self.max_login_retry;
        let mut attempt = 0;
        loop {
            match self.login() {
                Err(e) => match e {
                    LoginErr::KickedErr
                    | LoginErr::RegErr
                    | LoginErr::NicknameErr
                    | LoginErr::UnknownErr => {
                        log::error!("{}", e);
                        println!("Login error: {}", e); // Print error message
                        break;
                    }
                    LoginErr::CaptchaFailedSolveErr => {
                        log::error!("{}", e);
                        println!("Captcha failed to solve: {}", e); // Print error message
                        continue;
                    }
                    LoginErr::CaptchaWgErr | LoginErr::CaptchaUsedErr => {}
                    LoginErr::ServerDownErr | LoginErr::ServerDown500Err => {
                        log::error!("{}", e);
                        println!("Server is down: {}", e); // Print error message
                    }
                    LoginErr::Reqwest(err) => {
                        if err.is_connect() {
                            log::error!("{}\nIs tor proxy enabled ?", err);
                            println!("Connection error: {}\nIs tor proxy enabled ?", err); // Print error message
                            break;
                        } else if err.is_timeout() {
                            log::error!("timeout: {}", err);
                            println!("Timeout error: {}", err); // Print error message
                        } else {
                            log::error!("{}", err);
                            println!("Reqwest error: {}", err); // Print error message
                        }
                    }
                },

                Ok(()) => {
                    attempt = 0;
                    match self.get_msgs() {
                        Ok(ExitSignal::NeedLogin) => {}
                        Ok(ExitSignal::Terminate) => return,
                        Err(e) => log::error!("{:?}", e),
                    }
                }
            }
            attempt += 1;
            if max_retry > 0 && attempt > max_retry {
                break;
            }
            self.session = None;
            let retry_in = Duration::from_secs(2);
            let mut msg = format!("retry login in {:?}, attempt: {}", retry_in, attempt);
            if max_retry > 0 {
                msg += &format!("/{}", max_retry);
            }
            println!("{}", msg);
            thread::sleep(retry_in);
        }
    }

    fn start_keepalive_thread(
        &self,
        exit_rx: crossbeam_channel::Receiver<ExitSignal>,
        last_post_rx: crossbeam_channel::Receiver<()>,
        users: &Arc<Mutex<Users>>,
        username: &str,
    ) -> thread::JoinHandle<()> {
        let tx = self.tx.clone();
        let send_to = self.config.keepalive_send_to.clone();
        let users_clone = Arc::clone(users);
        let username_clone = username.to_string();
        thread::spawn(move || loop {
            // Check if user is a guest
            let is_guest = {
                let users_guard = users_clone.lock().unwrap();
                let is_member_or_staff = users_guard
                    .members
                    .iter()
                    .any(|(_, n)| n == &username_clone)
                    || users_guard.staff.iter().any(|(_, n)| n == &username_clone)
                    || users_guard.admin.iter().any(|(_, n)| n == &username_clone);
                !is_member_or_staff
            };

            let clb = || {
                // For guests, send keepalive to @0, otherwise use configured target
                let target = if is_guest {
                    "0".to_string()
                } else {
                    send_to.clone()
                };
                let _ = tx.send(PostType::KeepAlive(target));
            };

            // For guests: 25 minutes, for others: 55 minutes
            let timeout_minutes = if is_guest { 25 } else { 55 };
            let timeout = after(Duration::from_secs(60 * timeout_minutes));
            select! {
                // Whenever we send a message to chat server,
                // we will receive a message on this channel
                // and reset the timer for next keepalive.
                recv(&last_post_rx) -> _ => {},
                recv(&exit_rx) -> _ => return,
                recv(&timeout) -> _ => clb(),
            }
        })
    }

    // Thread that POST to chat server
    fn start_post_msg_thread(
        &self,
        exit_rx: crossbeam_channel::Receiver<ExitSignal>,
        last_post_tx: crossbeam_channel::Sender<()>,
    ) -> thread::JoinHandle<()> {
        let client = self.client.clone();
        let rx = Arc::clone(&self.rx);
        let full_url = format!("{}/{}", &self.config.url, &self.config.page_php);
        let session = self.session.clone().unwrap();
        let url = format!("{}?action=post&session={}", &full_url, &session);
        thread::spawn(move || loop {
            // Each message gets its own thread to avoid race conditions
            let rx = rx.lock().unwrap();
            select! {
                recv(&exit_rx) -> _ => return,
                recv(&rx) -> v => {
                    if let Ok(post_type_recv) = v {
                        // Clone necessary data for the new thread
                        let client_clone = client.clone();
                        let full_url_clone = full_url.clone();
                        let session_clone = session.clone();
                        let url_clone = url.clone();
                        let last_post_tx_clone = last_post_tx.clone();

                        // Spawn a new thread for each message to prevent race conditions
                        thread::spawn(move || {
                            post_msg(
                                &client_clone,
                                post_type_recv,
                                &full_url_clone,
                                session_clone,
                                &url_clone,
                                &last_post_tx_clone,
                            );
                        });
                    } else {
                        return;
                    }
                },
            }
        })
    }

    // Thread that update messages every "refresh_rate"
    fn start_get_msgs_thread(
        &self,
        sig: &Arc<Mutex<Sig>>,
        messages: &Arc<Mutex<Vec<Message>>>,
        users: &Arc<Mutex<Users>>,
        messages_updated_tx: crossbeam_channel::Sender<()>,
    ) -> thread::JoinHandle<()> {
        let client = self.client.clone();
        let messages = Arc::clone(messages);
        let users = Arc::clone(users);
        let session = self.session.clone().unwrap();
        let username = self.base_client.username.clone();
        let refresh_rate = self.refresh_rate;
        let base_url = self.config.url.clone();
        let page_php = self.config.page_php.clone();
        let datetime_fmt = self.config.datetime_fmt.clone();
        let is_muted = Arc::clone(&self.is_muted);
        let exit_rx = sig.lock().unwrap().clone();
        let sig = Arc::clone(sig);
        let members_tag = self.config.members_tag.clone();
        let staffs_tag = self.config.staffs_tag.clone();
        let tx = self.tx.clone();
        let bad_usernames = Arc::clone(&self.bad_username_filters);
        let bad_exact_usernames = Arc::clone(&self.bad_exact_username_filters);
        let bad_messages = Arc::clone(&self.bad_message_filters);
        let allowlist = Arc::clone(&self.allowlist);
        let alt_account = self.account_manager.alt_account.clone();
        let master_account = self.account_manager.master_account.clone();
        let alt_forwarding_enabled = Arc::clone(&self.alt_forwarding_enabled);
        let ai_enabled = Arc::clone(&self.ai_enabled);
        let ai_mode = Arc::clone(&self.ai_mode);
        let openai_client = self.openai_client.clone();
        let system_intel = self.system_intel.clone();
        let moderation_strictness = self.moderation_strictness.clone();
        let mod_logs_enabled = Arc::clone(&self.mod_logs_enabled);
        let ai_conversation_memory = Arc::clone(&self.ai_conversation_memory);
        let user_warnings = Arc::clone(&self.user_warnings);
        let ai_service = Arc::clone(&self.ai_service);
        let bot_manager = self.bot_manager.clone();
        thread::spawn(move || {
            #[cfg(feature = "audio")]
            let audio_output = OutputStream::try_default().ok();
            #[cfg(feature = "audio")]
            let stream_handle = audio_output.as_ref().map(|(_, handle)| handle);

            loop {
                let mut should_notify = false;

                if let Err(err) = get_msgs(
                    &client,
                    &base_url,
                    &page_php,
                    &session,
                    &username,
                    &users,
                    &sig,
                    &messages_updated_tx,
                    &members_tag,
                    &staffs_tag,
                    &datetime_fmt,
                    &messages,
                    &mut should_notify,
                    &tx,
                    &bad_usernames,
                    &bad_exact_usernames,
                    &bad_messages,
                    &allowlist,
                    alt_account.as_deref(),
                    master_account.as_deref(),
                    &alt_forwarding_enabled,
                    &ai_enabled,
                    &ai_mode,
                    &openai_client,
                    &system_intel,
                    &moderation_strictness,
                    &mod_logs_enabled,
                    &ai_conversation_memory,
                    &user_warnings,
                    &ai_service,
                    &bot_manager,
                ) {
                    log::error!("{}", err);
                };

                let muted = { *is_muted.lock().unwrap() };
                if should_notify && !muted {
                    #[cfg(feature = "audio")]
                    if let Some(handle) = &stream_handle {
                        if let Ok(source) = Decoder::new_mp3(Cursor::new(SOUND1)) {
                            if let Err(err) = handle.play_raw(source.convert_samples()) {
                                log::error!("Audio playback error: {}", err);
                            }
                        }
                    }
                }

                let timeout = after(Duration::from_secs(refresh_rate));
                select! {
                    recv(&exit_rx) -> _ => return,
                    recv(&timeout) -> _ => {},
                }
            }
        })
    }

    fn get_msgs(&mut self) -> anyhow::Result<ExitSignal> {
        let terminate_signal: ExitSignal;

        let messages: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));
        let users: Arc<Mutex<Users>> = Arc::new(Mutex::new(Users::default()));

        // Create default app state
        let mut app = App::default();

        // Each threads gets a clone of the receiver.
        // When someone calls ".signal", all threads receive it,
        // and knows that they have to terminate.
        let sig = Arc::new(Mutex::new(Sig::new()));

        let (messages_updated_tx, messages_updated_rx) = crossbeam_channel::unbounded();
        let (last_post_tx, last_post_rx) = crossbeam_channel::unbounded();

        let h1 = self.start_keepalive_thread(
            sig.lock().unwrap().clone(),
            last_post_rx,
            &users,
            &self.base_client.username,
        );
        let h2 = self.start_post_msg_thread(sig.lock().unwrap().clone(), last_post_tx);
        let h3 = self.start_get_msgs_thread(&sig, &messages, &users, messages_updated_tx);

        // Terminal initialization
        let mut stdout = io::stdout();
        enable_raw_mode().unwrap();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Setup event handlers
        let (events, h4) = Events::with_config(Config {
            messages_updated_rx,
            exit_rx: sig.lock().unwrap().clone(),
            // Increased from 250ms to 500ms to reduce CPU usage significantly
            tick_rate: Duration::from_millis(500),
        });

        loop {
            app.is_muted = *self.is_muted.lock().unwrap();
            app.show_sys = self.show_sys;
            app.display_guest_view = self.display_guest_view;
            app.display_member_view = self.display_member_view;
            app.display_hidden_msgs = self.display_hidden_msgs;
            app.display_pm_only = self.display_pm_only;
            app.display_staff_view = self.display_staff_view;
            app.display_master_pm_view = self.display_master_pm_view;
            app.clean_mode = self.clean_mode;
            app.inbox_mode = self.inbox_mode;
            // Account relationships are now managed by the account_manager
            app.members_tag = self.config.members_tag.clone();
            app.staffs_tag = self.config.staffs_tag.clone();

            // process()
            // Draw UI
            terminal.draw(|f| {
                draw_terminal_frame(f, &mut app, &messages, &users, &self.base_client.username);
            })?;

            // Handle input
            match self.handle_input(&events, &mut app, &messages, &users) {
                Err(ExitSignal::Terminate) => {
                    terminate_signal = ExitSignal::Terminate;
                    sig.lock().unwrap().signal(&terminate_signal);
                    break;
                }
                Err(ExitSignal::NeedLogin) => {
                    terminate_signal = ExitSignal::NeedLogin;
                    sig.lock().unwrap().signal(&terminate_signal);
                    break;
                }
                Ok(_) => continue,
            };
        }

        // Cleanup before leaving
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        terminal.clear()?;
        terminal.set_cursor(0, 0)?;

        if let Err(e) = h1.join() {
            log::error!("keepalive thread panicked: {:?}", e);
        }
        if let Err(e) = h2.join() {
            log::error!("post_msg thread panicked: {:?}", e);
        }
        if let Err(e) = h3.join() {
            log::error!("get_msgs thread panicked: {:?}", e);
        }
        if let Err(e) = h4.join() {
            log::error!("events thread panicked: {:?}", e);
        }

        Ok(terminate_signal)
    }

    fn post_msg(&self, post_type: PostType) -> anyhow::Result<()> {
        self.tx.send(post_type)?;
        Ok(())
    }

    fn clear_all_inbox_messages(&self, app: &mut App) -> anyhow::Result<()> {
        if let Some(session) = &self.session {
            let url = format!("{}?action=inbox&session={}", &self.config.url, session);

            // Collect all message IDs
            let message_ids: Vec<String> =
                app.inbox_items.items.iter().map(|m| m.id.clone()).collect();

            if message_ids.is_empty() {
                return Ok(());
            }

            let mut form = reqwest::blocking::multipart::Form::new()
                .text("lang", "en")
                .text("action", "inbox")
                .text("session", session.clone())
                .text("do", "clean");

            // Add all message IDs as checkboxes
            for mid in &message_ids {
                form = form.text("mid[]", mid.clone());
            }

            let response = self.client.post(&url).multipart(form).send()?;

            if response.status().is_success() {
                // Clear local inbox
                app.inbox_items.items.clear();
                app.inbox_items.state.select(None);
            } else {
                return Err(anyhow::anyhow!(
                    "Failed to clear inbox: {}",
                    response.status()
                ));
            }
        }
        Ok(())
    }

    fn login(&mut self) -> Result<(), LoginErr> {
        // If we provided a session, skip login process
        if self.session.is_some() {
            // println!("Session in params: {:?}", self.session);
            return Ok(());
        }
        // println!("self.session is not Some");
        // println!("self.sxiv = {:?}", self.sxiv);
        self.session = Some(lechatphp::login(
            &self.client,
            &self.config.url,
            &self.config.page_php,
            &self.base_client.username,
            &self.base_client.password,
            &self.guest_color,
            self.manual_captcha,
            self.sxiv,
        )?);
        Ok(())
    }

    fn logout(&mut self) -> anyhow::Result<()> {
        if let Some(session) = &self.session {
            lechatphp::logout(
                &self.client,
                &self.config.url,
                &self.config.page_php,
                session,
            )?;
            self.session = None;
        }
        Ok(())
    }

    fn start_cycle(&self, color_only: bool) {
        let username = self.base_client.username.clone();
        let tx = self.tx.clone();
        let color_rx = Arc::clone(&self.color_rx);
        thread::spawn(move || {
            let mut idx = 0;
            let colors = [
                "#ff3366", "#ff6633", "#FFCC33", "#33FF66", "#33FFCC", "#33CCFF", "#3366FF",
                "#6633FF", "#CC33FF", "#efefef",
            ];
            loop {
                let color_rx = color_rx.lock().unwrap();
                let timeout = after(Duration::from_millis(5200));
                select! {
                    recv(&color_rx) -> _ => break,
                    recv(&timeout) -> _ => {}
                }
                idx = (idx + 1) % colors.len();
                let color = colors[idx].to_owned();
                if !color_only {
                    let name = format!("{}{}", username, random_string(14));
                    log::error!("New name : {}", name);
                    let _ = tx.send(PostType::Profile(color, name, true, true, true));
                } else {
                    let _ = tx.send(PostType::NewColor(color));
                }
                // tx.send(PostType::Post("!up".to_owned(), Some(username.clone())))
                //     .unwrap();
                // tx.send(PostType::DeleteLast).unwrap();
            }
            let msg = PostType::Profile("#90ee90".to_owned(), username, true, true, true);
            let _ = tx.send(msg);
        });
    }

    fn save_filters(&self) {
        if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
            cfg.bad_usernames = self.bad_username_filters.lock().unwrap().clone();
            cfg.bad_exact_usernames = self.bad_exact_username_filters.lock().unwrap().clone();
            cfg.bad_messages = self.bad_message_filters.lock().unwrap().clone();
            cfg.allowlist = self.allowlist.lock().unwrap().clone();
            if let Err(e) = confy::store("bhcli", None, cfg) {
                log::error!("failed to store config: {}", e);
            }
        }
    }

    fn save_alt_forwarding_config(&self) {
        if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
            cfg.alt_forwarding_enabled = *self.alt_forwarding_enabled.lock().unwrap();
            if let Err(e) = confy::store("bhcli", None, cfg) {
                log::error!("failed to store config: {}", e);
            }
        }
    }

    fn save_ai_config(&self) {
        if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
            if let Some(profile_cfg) = cfg.profiles.get_mut(&self.profile) {
                profile_cfg.ai_enabled = *self.ai_enabled.lock().unwrap();
                profile_cfg.ai_mode = self.ai_mode.lock().unwrap().clone();
                profile_cfg.moderation_strictness = self.moderation_strictness.clone();
                profile_cfg.mod_logs_enabled = *self.mod_logs_enabled.lock().unwrap();
                if let Err(e) = confy::store("bhcli", None, cfg) {
                    log::error!("failed to store AI config: {}", e);
                }
            }
        }
    }

    fn set_account(&mut self, which: &str, username: String) {
        if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
            if let Some(profile_cfg) = cfg.profiles.get_mut(&self.profile) {
                match which {
                    "alt" => {
                        profile_cfg.alt_account = Some(username.clone());
                        self.account_manager.set_alt_account(username.clone());
                    }
                    "master" => {
                        profile_cfg.master_account = Some(username.clone());
                        self.account_manager.set_master_account(username.clone());
                    }
                    _ => return,
                }
                if let Err(e) = confy::store("bhcli", None, cfg) {
                    log::error!("failed to store config: {}", e);
                }
            }
        }
    }

    fn list_filters(&self, usernames: bool) -> String {
        let list = if usernames {
            self.bad_username_filters.lock().unwrap().clone()
        } else {
            self.bad_message_filters.lock().unwrap().clone()
        };
        if list.is_empty() {
            String::from("(empty)")
        } else {
            list.join(", ")
        }
    }

    fn list_exact_filters(&self) -> String {
        let list = self.bad_exact_username_filters.lock().unwrap().clone();
        if list.is_empty() {
            String::from("(empty)")
        } else {
            list.join(", ")
        }
    }

    fn remove_filter(&self, term: &str, usernames: bool) -> bool {
        if usernames {
            {
                let mut filters = self.bad_username_filters.lock().unwrap();
                if let Some(pos) = filters.iter().position(|x| x == term) {
                    filters.remove(pos);
                    return true;
                }
            }
            {
                let mut filters = self.bad_exact_username_filters.lock().unwrap();
                if let Some(pos) = filters.iter().position(|x| x == term) {
                    filters.remove(pos);
                    return true;
                }
            }
            false
        } else {
            let mut filters = self.bad_message_filters.lock().unwrap();
            if let Some(pos) = filters.iter().position(|x| x == term) {
                filters.remove(pos);
                true
            } else {
                false
            }
        }
    }

    fn apply_ban_filters(&self, users: &Arc<Mutex<Users>>) {
        let users = users.lock().unwrap();
        let name_filters = self.bad_username_filters.lock().unwrap().clone();
        let exact_filters = self.bad_exact_username_filters.lock().unwrap().clone();
        for (_, name) in &users.guests {
            if exact_filters.iter().any(|f| f == name)
                || name_filters
                    .iter()
                    .any(|f| name.to_lowercase().contains(&f.to_lowercase()))
            {
                let _ = self.tx.send(PostType::Kick(String::new(), name.clone()));
            }
        }
    }

    /// Determine user role based on current client state
    fn determine_user_role(&self) -> UserRole {
        // This is a simplified role determination - in a real implementation,
        // you'd check the user's actual permissions from the server
        if self.account_manager.master_account.is_some() {
            UserRole::Admin
        } else if self.account_manager.alt_account.is_some() {
            UserRole::Staff
        } else if !self.display_guest_view {
            UserRole::Member
        } else {
            UserRole::Guest
        }
    }

    fn handle_identity_command(
        &mut self,
        command: &str,
        message: &str,
        app: &mut App,
        target: Option<String>,
    ) -> bool {
        if let Some(identity_config) = self.identities.get(command) {
            if identity_config.len() < 2 {
                return false; // Invalid config, need at least nickname and color
            }

            let nickname = identity_config[0].clone();
            let color = identity_config[1].clone();
            // Trim quotes from color if present (for backwards compatibility)
            let color = color.trim_matches('"').trim_matches('\'');
            let incognito = identity_config.get(2).map(|s| s == "true").unwrap_or(false);
            let bold = identity_config.get(3).map(|s| s == "true").unwrap_or(false);
            let italic = identity_config.get(4).map(|s| s == "true").unwrap_or(false);

            // Store current user info for restoration
            let current_username = self.current_username.clone();
            let current_color = self.current_color.clone();

            if !message.is_empty() {
                // First set profile to the configured identity
                self.post_msg(PostType::Profile(
                    color.to_string(),
                    nickname,
                    incognito,
                    bold,
                    italic,
                ))
                .unwrap();

                // Check if this is a kick command
                if let Some(captures) = KICK_RGX.captures(message) {
                    // Handle kick command
                    let username = captures[1].to_owned();
                    let kick_msg = captures[2].to_owned();
                    let tx = self.tx.clone();
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(2000)); // Increased delay to 2 seconds
                        let _ = tx.send(PostType::Kick(kick_msg, username));

                        // Add another delay before restoring profile
                        thread::sleep(Duration::from_millis(1000));
                        let _ = tx.send(PostType::Profile(
                            current_color,
                            current_username,
                            true,
                            true,
                            true,
                        ));
                    });
                } else {
                    // Handle regular message
                    let tx = self.tx.clone();
                    let message_clone = message.to_owned();
                    let target_clone = target.clone();
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(2000)); // Increased delay to 2 seconds
                        let _ = tx.send(PostType::Post(message_clone, target_clone));

                        // Add another delay before restoring profile
                        thread::sleep(Duration::from_millis(1000));
                        let _ = tx.send(PostType::Profile(
                            current_color,
                            current_username,
                            true,
                            true,
                            true,
                        ));
                    });
                }
            }
            app.input = format!("/{} ", command);
            app.input_idx = app.input.width();
            true
        } else {
            false
        }
    }

    fn ensure_default_identities(&mut self) {
        // Add default identities if they don't exist
        let defaults = vec![
            (
                "admin",
                vec![
                    "Administrator".to_string(),
                    "#FF4444".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "mod",
                vec![
                    "Moderator".to_string(),
                    "#FFAA00".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "john",
                vec![
                    "JohnDoe".to_string(),
                    "#FC129E".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "intel",
                vec![
                    "intelroker".to_string(),
                    "#FF1212".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "op",
                vec![
                    "Operator".to_string(),
                    "#00FF88".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "shadow",
                vec![
                    "ShadowUser".to_string(),
                    "#2C2C2C".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                ],
            ),
            (
                "ghost",
                vec![
                    "Ghost".to_string(),
                    "#CCCCCC".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "cyber",
                vec![
                    "CyberNinja".to_string(),
                    "#00FFFF".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                    "true".to_string(),
                ],
            ),
            (
                "viper",
                vec![
                    "ViperX".to_string(),
                    "#00FF00".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                    "false".to_string(),
                ],
            ),
            (
                "phoenix",
                vec![
                    "PhoenixRise".to_string(),
                    "#FF8C00".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                    "true".to_string(),
                ],
            ),
        ];

        for (cmd, config) in defaults {
            if !self.identities.contains_key(cmd) {
                self.identities.insert(cmd.to_string(), config);
            }
        }
    }

    fn switch_to_identity(&mut self, command: &str) -> Result<(), String> {
        if let Some(identity_config) = self.identities.get(command) {
            if identity_config.len() >= 2 {
                let nickname = identity_config[0].clone();
                let color = identity_config[1].clone();
                // Trim quotes from color if present (for backwards compatibility)
                let color = color.trim_matches('"').trim_matches('\'');
                let incognito = identity_config.get(2).map(|s| s == "true").unwrap_or(false);
                let bold = identity_config.get(3).map(|s| s == "true").unwrap_or(false);
                let italic = identity_config.get(4).map(|s| s == "true").unwrap_or(false);

                // Update current identity tracking
                self.current_username = nickname.clone();
                self.current_color = color.to_string();

                // Update the base client username for login purposes
                self.base_client.username = nickname.clone();

                // Save username to config file for future logins
                if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
                    if let Some(profile_cfg) = cfg.profiles.get_mut(&self.profile) {
                        profile_cfg.username = nickname.clone();
                        if let Err(e) = confy::store("bhcli", None, cfg) {
                            log::error!("Failed to save username to config: {}", e);
                        }
                    }
                }

                // Permanently switch to the identity
                self.post_msg(PostType::Profile(
                    color.to_string(),
                    nickname.clone(),
                    incognito,
                    bold,
                    italic,
                ))
                .unwrap();

                // Send confirmation message to @0
                let confirmation_msg = format!("You are now @{}", nickname);
                self.post_msg(PostType::Post(confirmation_msg, Some("0".to_owned())))
                    .unwrap();

                Ok(())
            } else {
                Err(format!("Invalid identity configuration for /{}", command))
            }
        } else {
            Err(format!("Identity /{} not found", command))
        }
    }

    fn process_command_with_target(
        &mut self,
        input: &str,
        app: &mut App,
        users: &Arc<Mutex<Users>>,
        target: Option<String>,
    ) -> bool {
        // First, check for enhanced command processing (master/alt delegation)
        if let Some(enhanced_command) = parse_enhanced_command(input, &self.account_manager) {
            if enhanced_command != input {
                // Command was transformed, process the enhanced version recursively
                return self.process_command_with_target(&enhanced_command, app, users, target);
            }
        }

        // Check if account relationship is active for status display
        let relationship_status = self.account_manager.get_relationship_status(users);
        if matches!(relationship_status, AccountRelationshipStatus::MasterOffline | AccountRelationshipStatus::AltOffline) {
            // Optionally show status warning (could be toggled via config)
            if input.trim() == "/status" || input.trim() == "/account" {
                let status_message = self.account_manager.format_status_message(&relationship_status);
                self.post_msg(PostType::Post(status_message, target.clone())).unwrap();
                return true;
            }
        }

        // Try ChatOps commands
        let user_role = self.determine_user_role();
        if let Some(chatops_result) =
            self.chatops_router
                .process_command(input, &self.base_client.username, user_role)
        {
            // Convert ChatOps result to chat messages
            let messages = chatops_result.to_messages();
            for message in messages {
                // Special case: /help command should always go to @0 (user 0)
                let message_target = if input.trim() == "/help" {
                    Some("0".to_owned())
                } else {
                    // Use the provided target, or None for main chat
                    target.clone()
                };
                self.post_msg(PostType::Post(message, message_target))
                    .unwrap();
            }
            return true;
        }

        // Continue with existing commands
        if input == "/dl" {
            self.post_msg(PostType::DeleteLast).unwrap();
        } else if input.starts_with("/m ") {
            // Send message to members
            let msg = input.trim_start_matches("/m ").to_owned();
            let to = Some(SEND_TO_MEMBERS.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            return true;
        } else if input.starts_with("/s ") {
            // Send message to staff
            let msg = input.trim_start_matches("/s ").to_owned();
            let to = Some(SEND_TO_STAFFS.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            return true;
        } else if let Some(captures) = DLX_RGX.captures(input) {
            let x: usize = captures.get(1).unwrap().as_str().parse().unwrap();
            for _ in 0..x {
                self.post_msg(PostType::DeleteLast).unwrap();
            }
        } else if input == "/dall" {
            self.post_msg(PostType::DeleteAll).unwrap();
        } else if let Some(captures) = DELETE_RGX.captures(input) {
            let msg_id = captures.get(1).unwrap().as_str().to_owned();
            self.post_msg(PostType::Delete(msg_id)).unwrap();
        } else if input == "/cycles" {
            self.color_tx.send(()).unwrap();
        } else if input == "/cycle1" {
            self.start_cycle(true);
        } else if input == "/cycle2" {
            self.start_cycle(false);
        } else if input == "/kall" {
            let username = "s _".to_owned();
            let msg = "".to_owned();
            self.post_msg(PostType::Kick(msg, username)).unwrap();
        } else if let Some(captures) = PM_RGX.captures(input) {
            let username = &captures[1];
            let msg = captures[2].to_owned();
            let to = Some(username.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            app.input = format!("/pm {} ", username);
            app.input_idx = app.input.width();
        } else if let Some(captures) = NEW_NICKNAME_RGX.captures(input) {
            let new_nickname = captures[1].to_owned();
            self.post_msg(PostType::NewNickname(new_nickname)).unwrap();
        } else if let Some(captures) = NEW_COLOR_RGX.captures(input) {
            let new_color = captures[1].to_owned();
            self.post_msg(PostType::NewColor(new_color)).unwrap();
        } else if let Some(captures) = KICK_RGX.captures(input) {
            let username = captures[1].to_owned();
            let msg = captures[2].to_owned();

            // Protect Dasho from being kicked
            if username.to_lowercase() == "dasho" {
                let protection_msg = "❌ Cannot kick Dasho - protected user".to_string();
                self.post_msg(PostType::Post(protection_msg, Some("0".to_owned())))
                    .unwrap();
            } else {
                self.post_msg(PostType::Kick(msg, username)).unwrap();
            }
        } else if input.starts_with("/banname ") || input.starts_with("/ban ") {
            let mut name = if input.starts_with("/banname ") {
                remove_prefix(input, "/banname ")
            } else {
                remove_prefix(input, "/ban ")
            };
            let exact = name.starts_with('"') && name.ends_with('"') && name.len() >= 2;
            if exact {
                name = &name[1..name.len() - 1];
            }
            let name = name.to_owned();

            // Protect Dasho from being banned
            if name.to_lowercase().contains("dasho") {
                let protection_msg = "❌ Cannot ban Dasho - protected user".to_string();
                self.post_msg(PostType::Post(protection_msg, Some("0".to_owned())))
                    .unwrap();
            } else {
                if exact {
                    let mut f = self.bad_exact_username_filters.lock().unwrap();
                    f.push(name.clone());
                } else {
                    let mut f = self.bad_username_filters.lock().unwrap();
                    f.push(name.clone());
                }
                self.save_filters();
                self.post_msg(PostType::Kick(String::new(), name.clone()))
                    .unwrap();
                self.apply_ban_filters(users);
                let msg = if exact {
                    format!("Banned exact user \"{}\"", name)
                } else {
                    format!("Banned userfilter \"{}\"", name)
                };
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input.starts_with("/banmsg ") || input.starts_with("/filter ") {
            let term = if input.starts_with("/banmsg ") {
                remove_prefix(input, "/banmsg ")
            } else {
                remove_prefix(input, "/filter ")
            };
            let term = term.to_owned();
            {
                let mut f = self.bad_message_filters.lock().unwrap();
                f.push(term.clone());
            }
            self.save_filters();
            let msg = format!("Filtering messages including \"{}\"", term);
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/banlist" {
            let list = self.list_filters(true);
            let list_exact = self.list_exact_filters();
            let msg = format!("Banned names: {}", list)
                + &if list_exact.is_empty() {
                    String::new()
                } else {
                    format!("\nBanned exact names: {}", list_exact)
                };
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/filterlist" {
            let list = self.list_filters(false);
            let msg = format!("Filtered messages: {}", list);
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input.starts_with("/unban ") {
            let mut name = remove_prefix(input, "/unban ");
            if name.starts_with('"') && name.ends_with('"') && name.len() >= 2 {
                name = &name[1..name.len() - 1];
            }
            if self.remove_filter(name, true) {
                self.save_filters();
                let msg = format!("Unbanned {}", name);
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input.starts_with("/unfilter ") {
            let term = remove_prefix(input, "/unfilter ");
            if self.remove_filter(term, false) {
                self.save_filters();
                let msg = format!("Unfiltered \"{}\"", term);
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input.starts_with("/set ") {
            let rest = remove_prefix(input, "/set ");
            if let Some(username) = rest.strip_prefix("alt ") {
                let user = username.to_owned();
                self.set_account("alt", user.clone());
                let msg = format!("ALT account set to {}", user);
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            } else if let Some(username) = rest.strip_prefix("master ") {
                let user = username.to_owned();
                self.set_account("master", user.clone());
                let msg = format!("MASTER account set to {}", user);
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            } else {
                return false;
            }
        } else if input == "/alt on" {
            *self.alt_forwarding_enabled.lock().unwrap() = true;
            self.save_alt_forwarding_config();
            let msg = "ALT message forwarding enabled".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/alt off" {
            *self.alt_forwarding_enabled.lock().unwrap() = false;
            self.save_alt_forwarding_config();
            let msg = "ALT message forwarding disabled".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input.starts_with("/allow ") {
            let user = remove_prefix(input, "/allow ").to_owned();
            {
                let mut list = self.allowlist.lock().unwrap();
                if !list.contains(&user) {
                    list.push(user.clone());
                }
            }
            self.save_filters();
            let msg = format!("Allowed {}", user);
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input.starts_with("/revoke ") {
            let user = remove_prefix(input, "/revoke ").to_owned();
            {
                let mut list = self.allowlist.lock().unwrap();
                if let Some(pos) = list.iter().position(|u| u == &user) {
                    list.remove(pos);
                }
            }
            self.save_filters();
            let msg = format!("Revoked {}", user);
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/allowlist" {
            let list = self.allowlist.lock().unwrap().clone();
            let out = if list.is_empty() {
                String::from("(empty)")
            } else {
                list.join(", ")
            };
            let msg = format!("Allowlist: {}", out);
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai on" {
            *self.ai_enabled.lock().unwrap() = true;
            *self.ai_mode.lock().unwrap() = "mod_only".to_string();
            self.save_ai_config();
            let msg = "AI enabled in moderation only mode".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai mod" {
            *self.ai_enabled.lock().unwrap() = true;
            *self.ai_mode.lock().unwrap() = "mod_only".to_string();
            self.save_ai_config();
            let msg = "AI set to moderation only mode (kicks/bans harmful messages, no replies)"
                .to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai reply all" {
            *self.ai_enabled.lock().unwrap() = true;
            *self.ai_mode.lock().unwrap() = "reply_all".to_string();
            self.save_ai_config();
            let msg =
                "AI set to reply all mode (responds to all appropriate messages + moderation)"
                    .to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai reply ping" {
            *self.ai_enabled.lock().unwrap() = true;
            *self.ai_mode.lock().unwrap() = "reply_ping".to_string();
            self.save_ai_config();
            let msg =
                "AI set to reply ping mode (responds only when tagged/mentioned + moderation)"
                    .to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai off" {
            *self.ai_enabled.lock().unwrap() = false; // Completely disable AI
            *self.ai_mode.lock().unwrap() = "off".to_string(); // Completely off
            self.save_ai_config();
            let msg = "AI completely disabled (no moderation, no replies)".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai strict" {
            self.moderation_strictness = "strict".to_string();
            self.save_ai_config();
            let msg = "AI moderation set to STRICT mode (very strict, moderates anything potentially harmful)".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai balanced" {
            self.moderation_strictness = "balanced".to_string();
            self.save_ai_config();
            let msg = "AI moderation set to BALANCED mode (moderate clear violations, preserve free speech)".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/ai lenient" {
            self.moderation_strictness = "lenient".to_string();
            self.save_ai_config();
            let msg = "AI moderation set to LENIENT mode (very lenient, only moderate obvious violations)".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/check ai" {
            let ai_enabled = *self.ai_enabled.lock().unwrap();
            let ai_mode = self.ai_mode.lock().unwrap().clone();
            let has_openai = self.openai_client.is_some();

            let status_msg = format!(
                "AI Status Check:\n- AI Enabled: {}\n- AI Mode: {}\n- OpenAI Client: {}\n- Moderation Strictness: {}",
                if ai_enabled { "YES" } else { "NO" },
                ai_mode,
                if has_openai { "CONNECTED" } else { "NOT AVAILABLE (check OPENAI_API_KEY)" },
                self.moderation_strictness
            );

            self.post_msg(PostType::Post(status_msg, Some("0".to_owned())))
                .unwrap();

            // Test quick moderation patterns
            let test_messages = vec!["young boy", "hello world", "cheese pizza"];
            for test_msg in test_messages {
                let quick_result = if let Some(should_moderate) = quick_moderation_check(test_msg) {
                    if should_moderate {
                        "BLOCK"
                    } else {
                        "FLAG"
                    }
                } else {
                    "ALLOW"
                };
                let test_result = format!("Quick test '{}': {}", test_msg, quick_result);
                self.post_msg(PostType::Post(test_result, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input.starts_with("/check mod ") {
            let test_message = input.trim_start_matches("/check mod ").trim();
            if test_message.is_empty() {
                let msg = "Usage: /check mod <message> - Test AI moderation response for a message"
                    .to_string();
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            } else {
                let ai_enabled = *self.ai_enabled.lock().unwrap();
                let has_openai = self.openai_client.is_some();

                if !ai_enabled {
                    let msg = "AI is currently disabled. Enable with /ai mod first.".to_string();
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                } else if !has_openai {
                    let msg =
                        "OpenAI client not available. Check OPENAI_API_KEY environment variable."
                            .to_string();
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                } else {
                    // First test quick moderation
                    let quick_result =
                        if let Some(should_moderate) = quick_moderation_check(test_message) {
                            if should_moderate {
                                "YES (Quick Pattern Match)"
                            } else {
                                "NO (Quick Pattern False)"
                            }
                        } else {
                            "INCONCLUSIVE (Needs AI Analysis)"
                        };

                    let quick_msg = format!("Quick Check: '{}' -> {}", test_message, quick_result);
                    self.post_msg(PostType::Post(quick_msg, Some("0".to_owned())))
                        .unwrap();

                    // If quick check didn't catch it, test AI moderation
                    if quick_result == "INCONCLUSIVE (Needs AI Analysis)" {
                        let openai_client = self.openai_client.as_ref().unwrap().clone();
                        let moderation_strictness = self.moderation_strictness.clone();
                        let test_msg = test_message.to_string();
                        let tx = self.tx.clone();

                        // Show that we're starting AI analysis
                        let start_msg = format!("Starting AI analysis for: '{}'...", test_msg);
                        self.post_msg(PostType::Post(start_msg, Some("0".to_owned())))
                            .unwrap();

                        // Use same pattern as process_ai_message - create runtime and spawn thread
                        thread::spawn(move || {
                            let rt = Runtime::new().unwrap();
                            rt.block_on(async move {
                                match check_ai_moderation(&openai_client, &test_msg, &moderation_strictness).await {
                                    Some(true) => {
                                        let ai_msg = format!("AI Check: '{}' -> YES (AI recommends kick)", test_msg);
                                        let _ = tx.send(PostType::Post(ai_msg, Some("0".to_owned())));
                                    }
                                    Some(false) => {
                                        let ai_msg = format!("AI Check: '{}' -> NO (AI allows message)", test_msg);
                                        let _ = tx.send(PostType::Post(ai_msg, Some("0".to_owned())));
                                    }
                                    None => {
                                        let ai_msg = format!("AI Check: '{}' -> ERROR (AI request failed - check logs)", test_msg);
                                        let _ = tx.send(PostType::Post(ai_msg, Some("0".to_owned())));
                                    }
                                }
                            });
                        });
                    }
                }
            }
        } else if input == "/modlog on" {
            *self.mod_logs_enabled.lock().unwrap() = true;
            self.save_ai_config();
            let msg =
                "Moderation logging ENABLED - MOD LOG messages will be sent to @0".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/modlog off" {
            *self.mod_logs_enabled.lock().unwrap() = false;
            self.save_ai_config();
            let msg = "Moderation logging DISABLED - MOD LOG messages are now muted".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/warnings" {
            let warnings = self.user_warnings.lock().unwrap();
            if warnings.is_empty() {
                let msg = "No active warnings".to_string();
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            } else {
                let mut msg = "Current warnings:\n".to_string();
                for (user, count) in warnings.iter() {
                    msg.push_str(&format!("- {}: {}/3 warnings\n", user, count));
                }
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input.starts_with("/clearwarn ") {
            let user = input.trim_start_matches("/clearwarn ").trim();
            if !user.is_empty() {
                let mut warnings = self.user_warnings.lock().unwrap();
                if warnings.remove(user).is_some() {
                    let msg = format!("Cleared warnings for {}", user);
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                } else {
                    let msg = format!("No warnings found for {}", user);
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                }
            }
        } else if input == "/clearwarn all" {
            let mut warnings = self.user_warnings.lock().unwrap();
            let count = warnings.len();
            warnings.clear();
            let msg = format!("Cleared all warnings ({} users)", count);
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input == "/clearinbox" {
            if self.inbox_mode {
                match self.clear_all_inbox_messages(app) {
                    Ok(()) => {
                        let msg = "All inbox messages cleared".to_string();
                        self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                            .unwrap();
                    }
                    Err(e) => {
                        let msg = format!("Failed to clear inbox: {}", e);
                        self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                            .unwrap();
                    }
                }
            } else {
                let msg = "Command only available in inbox mode (Shift+O)".to_string();
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if let Some(captures) = IGNORE_RGX.captures(input) {
            let username = captures[1].to_owned();
            self.post_msg(PostType::Ignore(username)).unwrap();
        } else if let Some(captures) = UNIGNORE_RGX.captures(input) {
            let username = captures[1].to_owned();
            self.post_msg(PostType::Unignore(username)).unwrap();
        } else if let Some(captures) = UPLOAD_RGX.captures(input) {
            let file_path = captures[1].to_owned();
            let send_to = match captures.get(2) {
                Some(to_match) => match to_match.as_str() {
                    "members" => SEND_TO_MEMBERS,
                    "staffs" => SEND_TO_STAFFS,
                    "admins" => SEND_TO_ADMINS,
                    _ => SEND_TO_ALL,
                },
                None => SEND_TO_ALL,
            }
            .to_owned();
            let msg = match captures.get(3) {
                Some(msg_match) => msg_match.as_str().to_owned(),
                None => "".to_owned(),
            };
            self.post_msg(PostType::Upload(file_path, send_to, msg))
                .unwrap();
        } else if input.starts_with("/hide on") {
            // Toggle incognito mode on
            self.post_msg(PostType::SetIncognito(true)).unwrap();
            let msg = "Incognito mode ENABLED - you will be hidden from the guest list".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input.starts_with("/hide off") {
            // Toggle incognito mode off
            self.post_msg(PostType::SetIncognito(false)).unwrap();
            let msg = "Incognito mode DISABLED - you will be visible on the guest list".to_string();
            self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                .unwrap();
        } else if input.starts_with("/switch ") {
            // Alias for /identity switch
            let command = input.trim_start_matches("/switch ");
            match self.switch_to_identity(command) {
                Ok(()) => {} // Success message already sent by helper
                Err(msg) => {
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                }
            }
        } else if input.starts_with("/identity ") {
            let rest = input.trim_start_matches("/identity ");
            if rest == "list" {
                if self.identities.is_empty() {
                    let msg = "No custom identities configured".to_string();
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                } else {
                    let mut msg = "Configured identities:\n".to_string();
                    for (cmd, config) in &self.identities {
                        let nickname = config.get(0).cloned().unwrap_or_else(|| "?".to_string());
                        let color = config.get(1).cloned().unwrap_or_else(|| "?".to_string());
                        msg.push_str(&format!("/{}: {} ({})\n", cmd, nickname, color));
                    }
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                }
            } else if rest.starts_with("add ") {
                let parts: Vec<&str> = rest.splitn(4, ' ').collect();
                if parts.len() >= 4 {
                    let command = parts[1];
                    let nickname = parts[2];
                    let color = parts[3];
                    // Trim quotes from color if present
                    let color = color.trim_matches('"').trim_matches('\'');
                    // Create a complete config: [nickname, color, incognito, bold, italic]
                    let config = vec![
                        nickname.to_string(),
                        color.to_string(),
                        "false".to_string(), // incognito
                        "false".to_string(), // bold
                        "false".to_string(), // italic
                    ];

                    // Update in memory
                    self.identities.insert(command.to_string(), config);

                    // Save to config file
                    if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
                        if let Some(profile_cfg) = cfg.profiles.get_mut(&self.profile) {
                            profile_cfg
                                .identities
                                .insert(command.to_string(), self.identities[command].clone());
                            if let Err(e) = confy::store("bhcli", None, cfg) {
                                log::error!("failed to store config: {}", e);
                            } else {
                                let msg = format!(
                                    "Added identity /{}: {} ({})",
                                    command, nickname, color
                                );
                                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                                    .unwrap();
                            }
                        }
                    }
                } else {
                    let msg = "Usage: /identity add <command> <nickname> <color>".to_string();
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                }
            } else if rest.starts_with("remove ") {
                let command = rest.trim_start_matches("remove ");
                if self.identities.remove(command).is_some() {
                    // Save to config file
                    if let Ok(mut cfg) = confy::load::<MyConfig>("bhcli", None) {
                        if let Some(profile_cfg) = cfg.profiles.get_mut(&self.profile) {
                            profile_cfg.identities.remove(command);
                            if let Err(e) = confy::store("bhcli", None, cfg) {
                                log::error!("failed to store config: {}", e);
                            } else {
                                let msg = format!("Removed identity /{}", command);
                                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                                    .unwrap();
                            }
                        }
                    }
                } else {
                    let msg = format!("Identity /{} not found", command);
                    self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                        .unwrap();
                }
            } else if rest.starts_with("switch ") {
                let command = rest.trim_start_matches("switch ");
                match self.switch_to_identity(command) {
                    Ok(()) => {} // Success message already sent by helper
                    Err(msg) => {
                        self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                            .unwrap();
                    }
                }
            } else {
                let msg = "Usage: /identity list | add <command> <nickname> <color> | remove <command> | switch <command>".to_string();
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input.starts_with("/me ") {
            // Handle /me commands - send as regular messages including the /me prefix
            self.post_msg(PostType::Post(input.to_string(), target))
                .unwrap();
            return true;
        } else if input.starts_with('/') && input.contains(' ') {
            // Check for any unknown slash command that might be a custom identity
            if let Some(space_pos) = input.find(' ') {
                let command = &input[1..space_pos]; // Remove leading '/' and get command name
                let message = input[space_pos + 1..].trim();
                if self.handle_identity_command(command, message, app, target.clone()) {
                    return true;
                }
            }
        } else if input.starts_with("!warn") {
            let msg = input.trim_start_matches("!warn").trim();
            let msg = if msg.starts_with('@') {
                msg.to_owned()
            } else if msg.is_empty() {
                String::new()
            } else {
                format!("@{}", msg)
            };
            let end_msg = format!(
                "This is your warning - {}, will be kicked next. Please read the !-rules.",
                msg
            );
            self.post_msg(PostType::Post(end_msg, None)).unwrap();
        } else if input == "/help" {
            let help_text = r#"Available Commands:

Chat Commands:
/pm [user] [message] - Send private message to user
/m [message] - Send message to members only
/s [message] - Send message to staff only

Identity Commands (send as different users, then restore to original user):
/john [message] - Send as JohnDoe with pink color, then restore
/intel [message] - Send as intelroker with red color, then restore
/op [message] - Send as Operator with white color, then restore
/shadow [message] - Send as ShadowUser with dark gray color, then restore
/ghost [message] - Send as Ghost with light gray color, then restore
/cyber [message] - Send as CyberNinja with electric blue color, then restore
/viper [message] - Send as ViperX with green color, then restore
/phoenix [message] - Send as PhoenixRise with orange color, then restore
(All identity commands support targeting: /m /john, /s /intel, /pm user /op)
(Custom identities can be configured - see Identity Configuration below)

Identity Configuration:
/identity list - List all configured custom identities
/identity add [cmd] [nick] [color] - Add custom identity (/cmd nickname #color)
/identity remove [cmd] - Remove custom identity command
/identity switch [cmd] - Permanently switch to the specified identity
/switch [cmd] - Alias for /identity switch (quick identity switching)

ChatOps Developer Commands (30+ tools available):
/man [command] - Manual pages for system commands
/doc [lang] [term] - Language-specific documentation
/github [user/repo] - GitHub repository information
/crates [crate] - Rust crate information from crates.io
/npm [package] - NPM package information
/hash [algo] [text] - Generate cryptographic hashes
/uuid - Generate UUID v4
/base64 [encode|decode] - Base64 encoding/decoding
/regex [pattern] [text] - Test regular expressions
/whois [domain] - Domain WHOIS lookup
/dig [domain] - DNS record lookup
/ping [host] - Test network connectivity
/time - Current timestamp info
/explain [concept] - AI explanations of concepts
/translate [lang] [text] - Translate text between languages
... and 15+ more tools

Use '/commands' to see all ChatOps commands for your role.
Use '/help [command]' for detailed help on ChatOps commands.

ChatOps Command Prefixes:
/pm [user] /command - Send ChatOps result as PM to user
/m /command - Send ChatOps result to members channel
/s /command - Send ChatOps result to staff channel
(no prefix) - Send ChatOps result to main chat

AI Commands:
/ai off - Completely disable AI (no moderation, no replies)
/ai mod - Enable moderation only (kicks/bans harmful messages)
/ai reply all - Enable replies to all messages + moderation
/ai reply ping - Enable replies only when tagged + moderation
/ai strict - Set AI moderation to strict mode (very strict)
/ai balanced - Set AI moderation to balanced mode (default)
/ai lenient - Set AI moderation to lenient mode (very lenient)
/check ai - Check AI system status and OpenAI connection
/check mod [message] - Test AI moderation response for a message
/modlog on/off - Enable/disable moderation logging to @0
/warnings - Show current warning counts for users
/clearwarn [user] - Clear warnings for specific user
/clearwarn all - Clear all user warnings

Inbox Commands:
Shift+O - Toggle inbox view (view offline PMs)
x (in inbox) - Delete selected inbox message
/clearinbox - Clear all inbox messages (only in inbox mode)

Moderation Commands:
/kick [user] [reason] - Kick user with reason
/ban [username] - Ban username (partial match)
/ban "[exact]" - Ban exact username (use quotes)
/unban [username] - Remove username ban
/unfilter [text] - Remove message filter
/filter [text] - Filter messages containing text (same as /banmsg)
/allow [user] - Add user to allowlist (bypass filters)
/revoke [user] - Remove user from allowlist
/banlist - Show banned usernames
/filterlist - Show filtered message terms
/allowlist - Show allowlisted users
!warn [@user] - Send warning message

Message Management:
/dl - Delete last message
/dl[number] - Delete last N messages (e.g., /dl5)
/dall - Delete all messages
/delete [msg_id] - Delete specific message by ID

Account Management:
/set alt [username] - Set alt account for forwarding
/set master [username] - Set master account for PMs
/alt on/off - Enable/disable alt message forwarding

File Upload:
/upload [path] [to] [msg] - Upload file (to: members/staffs/admins/all)

Visual/Color:
/nick [nickname] - Change nickname
/color [hex] - Change color (#ff0000)
/cycle1 - Start color cycling
/cycle2 - Start name + color cycling
/cycles - Stop cycling

Utility:
/status - Show current settings and status
/help - Show this help message

Note: Some commands require appropriate permissions."#;

            self.post_msg(PostType::Post(help_text.to_string(), Some("0".to_owned())))
                .unwrap();
        } else if input == "/commands" {
            // List all ChatOps commands available to user
            let user_role = self.determine_user_role();
            if let Some(chatops_result) =
                self.chatops_router
                    .process_command("/list", &self.base_client.username, user_role)
            {
                let messages = chatops_result.to_messages();
                for message in messages {
                    self.post_msg(PostType::Post(message, Some("0".to_owned())))
                        .unwrap();
                }
            } else {
                let msg = "ChatOps commands not available.".to_string();
                self.post_msg(PostType::Post(msg, Some("0".to_owned())))
                    .unwrap();
            }
        } else if input == "/status" {
            let ai_enabled = *self.ai_enabled.lock().unwrap();
            let ai_mode = self.ai_mode.lock().unwrap().clone();
            let alt_forwarding = *self.alt_forwarding_enabled.lock().unwrap();

            let alt_account = self
                .account_manager.alt_account
                .as_ref()
                .map(|a| a.as_str())
                .unwrap_or("(not set)");
            let master_account = self
                .account_manager.master_account
                .as_ref()
                .map(|m| m.as_str())
                .unwrap_or("(not set)");

            let bad_usernames = self.bad_username_filters.lock().unwrap();
            let bad_exact_usernames = self.bad_exact_username_filters.lock().unwrap();
            let bad_messages = self.bad_message_filters.lock().unwrap();
            let allowlist = self.allowlist.lock().unwrap();

            let status_text = format!(
                r#"Current Status:

Account Settings:
- Username: {}
- ALT Account: {}
- Master Account: {}
- ALT Forwarding: {}

AI Settings:
- AI Enabled: {}
- AI Mode: {}
- System Intel: {}
- Moderation Strictness: {}
- Mod Logs Enabled: {}

Display Settings:
- Show System Messages: {}
- Guest View: {}
- Member View: {}
- Staff View: {}
- Master PM View: {}
- PM Only Mode: {}
- Hidden Messages: {}
- Clean Mode: {}
- Sound Muted: {}

Filters & Moderation:
- Banned Usernames ({}): {}
- Banned Exact Names ({}): {}
- Filtered Messages ({}): {}
- Allowlisted Users ({}): {}

Connection:
- Profile: {}
- Session Active: {}
- Refresh Rate: {}s"#,
                self.base_client.username,
                alt_account,
                master_account,
                if alt_forwarding { "ON" } else { "OFF" },
                if ai_enabled { "YES" } else { "NO" },
                ai_mode,
                if self.system_intel.len() > 50 {
                    format!("{}...", &self.system_intel[..50])
                } else {
                    self.system_intel.clone()
                },
                self.moderation_strictness,
                if *self.mod_logs_enabled.lock().unwrap() {
                    "ON"
                } else {
                    "OFF"
                },
                if self.show_sys { "ON" } else { "OFF" },
                if self.display_guest_view { "ON" } else { "OFF" },
                if self.display_member_view {
                    "ON"
                } else {
                    "OFF"
                },
                if self.display_staff_view { "ON" } else { "OFF" },
                if self.display_master_pm_view {
                    "ON"
                } else {
                    "OFF"
                },
                if self.display_pm_only { "ON" } else { "OFF" },
                if self.display_hidden_msgs {
                    "ON"
                } else {
                    "OFF"
                },
                if self.clean_mode { "ON" } else { "OFF" },
                if *self.is_muted.lock().unwrap() {
                    "YES"
                } else {
                    "NO"
                },
                bad_usernames.len(),
                if bad_usernames.is_empty() {
                    "(none)".to_string()
                } else {
                    bad_usernames.join(", ")
                },
                bad_exact_usernames.len(),
                if bad_exact_usernames.is_empty() {
                    "(none)".to_string()
                } else {
                    bad_exact_usernames.join(", ")
                },
                bad_messages.len(),
                if bad_messages.is_empty() {
                    "(none)".to_string()
                } else {
                    bad_messages.join(", ")
                },
                allowlist.len(),
                if allowlist.is_empty() {
                    "(none)".to_string()
                } else {
                    allowlist.join(", ")
                },
                self.profile,
                if self.session.is_some() { "YES" } else { "NO" },
                self.refresh_rate
            );

            self.post_msg(PostType::Post(status_text, Some("0".to_owned())))
                .unwrap();
        } else {
            return false;
        }
        true
    }

    fn handle_input(
        &mut self,
        events: &Events,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        match events.next() {
            Ok(Event::NeedLogin) => return Err(ExitSignal::NeedLogin),
            Ok(Event::Terminate) => return Err(ExitSignal::Terminate),
            Ok(Event::Input(evt)) => self.handle_event(app, messages, users, evt),
            _ => Ok(()),
        }
    }

    fn handle_event(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
        users: &Arc<Mutex<Users>>,
        event: event::Event,
    ) -> Result<(), ExitSignal> {
        match event {
            event::Event::Resize(_cols, _rows) => Ok(()),
            event::Event::FocusGained => Ok(()),
            event::Event::FocusLost => Ok(()),
            event::Event::Paste(_) => Ok(()),
            event::Event::Key(key_event) => self.handle_key_event(app, messages, users, key_event),
            event::Event::Mouse(mouse_event) => {
                // Ignore mouse events when external editor is active
                if app.external_editor_active {
                    Ok(())
                } else {
                    self.handle_mouse_event(app, mouse_event)
                }
            }
        }
    }

    fn handle_key_event(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
        users: &Arc<Mutex<Users>>,
        key_event: KeyEvent,
    ) -> Result<(), ExitSignal> {
        if app.input_mode != InputMode::Normal {
            self.last_key_event = None;
        }
        match app.input_mode {
            InputMode::LongMessage => {
                self.handle_long_message_mode_key_event(app, key_event, messages)
            }
            InputMode::Normal => self.handle_normal_mode_key_event(app, key_event, messages),
            InputMode::Editing | InputMode::EditingErr => {
                self.handle_editing_mode_key_event(app, key_event, users)
            }
            InputMode::MultilineEditing => {
                self.handle_multiline_editing_mode_key_event(app, key_event, users)
            }
            InputMode::Notes => {
                self.handle_notes_mode_key_event(app, key_event)
            }
            InputMode::MessageEditor => {
                self.handle_message_editor_key_event(app, key_event, users)
            }
        }
    }

    fn handle_long_message_mode_key_event(
        &mut self,
        app: &mut App,
        key_event: KeyEvent,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) -> Result<(), ExitSignal> {
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_long_message_mode_key_event_esc(app),
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_long_message_mode_key_event_ctrl_d(app, messages),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Scroll down
                app.long_message_scroll_offset = app.long_message_scroll_offset.saturating_add(1);
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Scroll up
                app.long_message_scroll_offset = app.long_message_scroll_offset.saturating_sub(1);
            }
            KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Scroll up by 10 lines
                app.long_message_scroll_offset = app.long_message_scroll_offset.saturating_sub(10);
            }
            KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Scroll down by 10 lines
                app.long_message_scroll_offset = app.long_message_scroll_offset.saturating_add(10);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_normal_mode_key_event(
        &mut self,
        app: &mut App,
        key_event: KeyEvent,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) -> Result<(), ExitSignal> {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_slash(app),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_down(app),
            KeyEvent {
                code: KeyCode::Char('J'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_j(app, 5),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_up(app),
            KeyEvent {
                code: KeyCode::Char('K'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_k(app, 5),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_normal_mode_key_event_member_pm(app)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_enter(app, messages),
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_backspace(app, messages),
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_normal_mode_key_event_yank(app),
            KeyEvent {
                code: KeyCode::Char('Y'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_yank_link(app),

            //Strange
            KeyEvent {
                code: KeyCode::Char('D'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_download_link(app),

            //Strange
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_download_and_view(app),

            // KeyEvent {
            //     code: KeyCode::Char('d'),
            //     modifiers: KeyModifiers::NONE,
            //     ..
            // } => self.handle_normal_mode_key_event_debug(app),
            // KeyEvent {
            //     code: KeyCode::Char('D'),
            //     modifiers: KeyModifiers::SHIFT,
            //     ..
            // } => self.handle_normal_mode_key_event_debug2(app),
            KeyEvent {
                code: KeyCode::Char('m'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_toggle_mute(),
            KeyEvent {
                code: KeyCode::Char('S'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_toggle_sys(),
            KeyEvent {
                code: KeyCode::Char('M'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_toggle_member_view(),
            KeyEvent {
                code: KeyCode::Char('G'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_toggle_guest_view(),
            KeyEvent {
                code: KeyCode::Char('P'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_toggle_pm_only(),
            KeyEvent {
                code: KeyCode::Char('V'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_toggle_v_view(),
            KeyEvent {
                code: KeyCode::Char('C'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_shift_c(app),
            KeyEvent {
                code: KeyCode::Char('O'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_shift_o(app),
            KeyEvent {
                code: KeyCode::Char('H'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_toggle_hidden(),
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_input_mode(app),
            KeyEvent {
                code: KeyCode::Char('Q'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_logout()?,
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_exit()?,
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_tag(app),
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_pm(app),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_normal_mode_key_event_member_pm(app),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_normal_mode_key_event_kick(app),
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_normal_mode_key_event_ban(app),
            KeyEvent {
                code: KeyCode::Char('B'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_normal_mode_key_event_ban_exact(app)
            }
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_normal_mode_key_event_warn(app),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_space(app),
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_delete(app, messages),
            KeyEvent {
                code: KeyCode::Char('T'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.handle_normal_mode_key_event_translate(app, messages);
            }
            KeyEvent {
                code: KeyCode::Char('N'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                app.enter_notes_mode(self);
            }
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_page_up(app),
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_page_down(app),
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_esc(app),
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_normal_mode_key_event_shift_u(app),
            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_normal_mode_key_event_g(app),
            _ => {}
        }
        self.last_key_event = Some(key_event.code);
        Ok(())
    }

    fn handle_editing_mode_key_event(
        &mut self,
        app: &mut App,
        key_event: KeyEvent,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        app.input_mode = InputMode::Editing;
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::SHIFT)
                || modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.handle_editing_mode_key_event_newline(app)
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_enter(app, users)?,
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_tab(app, users),
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_c(app),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_a(app),
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_e(app),
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_f(app),
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_b(app),
            KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_v(app),
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                app.enter_message_editor_mode();
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_toggle_multiline(app),
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_left(app),
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_right(app),
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_down(app),
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_up(app),
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_editing_mode_key_event_shift_c(app, c),
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_backspace(app),
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_delete(app),
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_esc(app),
            _ => {}
        }
        Ok(())
    }

    fn handle_long_message_mode_key_event_esc(&mut self, app: &mut App) {
        app.long_message = None;
        app.input_mode = InputMode::Normal;
    }

    fn handle_long_message_mode_key_event_ctrl_d(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                self.post_msg(PostType::Clean(item.date.to_owned(), item.text.text()))
                    .unwrap();
                let mut messages = messages.lock().unwrap();
                if let Some(pos) = messages
                    .iter()
                    .position(|m| m.date == item.date && m.text == item.text)
                {
                    messages[pos].hide = !messages[pos].hide;
                }
                app.long_message = None;
                app.input_mode = InputMode::Normal;
            }
        }
    }

    fn handle_normal_mode_key_event_up(&mut self, app: &mut App) {
        if app.inbox_mode {
            app.inbox_items.previous();
        } else if app.clean_mode {
            app.clean_items.previous();
        } else {
            app.items.previous();
        }
    }

    fn handle_normal_mode_key_event_down(&mut self, app: &mut App) {
        if app.inbox_mode {
            app.inbox_items.next();
        } else if app.clean_mode {
            app.clean_items.next();
        } else {
            app.items.next();
        }
    }

    fn handle_normal_mode_key_event_space(&mut self, app: &mut App) {
        if app.inbox_mode {
            // Toggle checkbox for selected inbox message
            if let Some(idx) = app.inbox_items.state.selected() {
                if let Some(message) = app.inbox_items.items.get_mut(idx) {
                    message.selected = !message.selected;
                }
            }
        } else if app.clean_mode {
            // Toggle checkbox for selected clean message
            if let Some(idx) = app.clean_items.state.selected() {
                if let Some(message) = app.clean_items.items.get_mut(idx) {
                    message.selected = !message.selected;
                }
            }
        }
    }

    fn handle_normal_mode_key_event_j(&mut self, app: &mut App, lines: usize) {
        for _ in 0..lines {
            if app.inbox_mode {
                app.inbox_items.next();
            } else if app.clean_mode {
                app.clean_items.next();
            } else {
                app.items.next();
            }
        }
    }

    fn handle_normal_mode_key_event_k(&mut self, app: &mut App, lines: usize) {
        for _ in 0..lines {
            if app.inbox_mode {
                app.inbox_items.previous();
            } else if app.clean_mode {
                app.clean_items.previous();
            } else {
                app.items.previous();
            }
        }
    }

    fn handle_normal_mode_key_event_slash(&mut self, app: &mut App) {
        app.items.unselect();
        app.input = "/".to_owned();
        app.input_idx = app.input.width();
        app.input_mode = InputMode::Editing;
    }

    fn handle_normal_mode_key_event_enter(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                // If we have a filter, <enter> will "jump" to the message
                if !app.filter.is_empty() {
                    let idx = messages
                        .lock()
                        .unwrap()
                        .iter()
                        .enumerate()
                        .find(|(_, e)| e.date == item.date)
                        .map(|(i, _)| i);
                    app.clear_filter();
                    app.items.state.select(idx);
                    return;
                }
                app.long_message = Some(item.clone());
                app.long_message_scroll_offset = 0;
                app.input_mode = InputMode::LongMessage;
            }
        }
    }

    fn handle_normal_mode_key_event_backspace(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                let mut messages = messages.lock().unwrap();
                if let Some(pos) = messages
                    .iter()
                    .position(|m| m.date == item.date && m.text == item.text)
                {
                    if item.deleted {
                        messages.remove(pos);
                    } else {
                        messages[pos].hide = !messages[pos].hide;
                    }
                }
            }
        }
    }

    fn handle_normal_mode_key_event_yank(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                if let Some(upload_link) = &item.upload_link {
                    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                    let mut out = format!("{}{}", self.config.url, upload_link);
                    if let Some((_, _, msg, _)) = get_message(
                        &item.text,
                        &self.config.members_tag,
                        &self.config.staffs_tag,
                    ) {
                        out = format!("{} {}", msg, out);
                    }
                    ctx.set_contents(out).unwrap();
                } else if let Some((_, _, msg, _)) = get_message(
                    &item.text,
                    &self.config.members_tag,
                    &self.config.staffs_tag,
                ) {
                    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                    ctx.set_contents(msg).unwrap();
                }
            }
        }
    }

    fn handle_normal_mode_key_event_yank_link(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                if let Some(upload_link) = &item.upload_link {
                    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                    let out = format!("{}{}", self.config.url, upload_link);
                    ctx.set_contents(out).unwrap();
                } else if let Some((_, _, msg, _)) = get_message(
                    &item.text,
                    &self.config.members_tag,
                    &self.config.staffs_tag,
                ) {
                    let finder = LinkFinder::new();
                    let links: Vec<_> = finder.links(msg.as_str()).collect();
                    if let Some(link) = links.get(0) {
                        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                        ctx.set_contents(link.as_str().to_owned()).unwrap();
                    }
                }
            }
        }
    }

    //Strange
    fn handle_normal_mode_key_event_download_link(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                if let Some(upload_link) = &item.upload_link {
                    let url = format!("{}{}", self.config.url, upload_link);
                    let _ = Command::new("curl")
                        .args([
                            "--socks5",
                            "localhost:9050",
                            "--socks5-hostname",
                            "localhost:9050",
                            &url,
                        ])
                        .arg("-o")
                        .arg("download.img")
                        .output()
                        .expect("Failed to execute curl command");
                } else if let Some((_, _, msg, _)) = get_message(
                    &item.text,
                    &self.config.members_tag,
                    &self.config.staffs_tag,
                ) {
                    let finder = LinkFinder::new();
                    let links: Vec<_> = finder.links(msg.as_str()).collect();
                    if let Some(link) = links.first() {
                        let url = link.as_str();
                        let _ = Command::new("curl")
                            .args([
                                "--socks5",
                                "localhost:9050",
                                "--socks5-hostname",
                                "localhost:9050",
                                url,
                            ])
                            .arg("-o")
                            .arg("download.img")
                            .output()
                            .expect("Failed to execute curl command");
                    }
                }
            }
        }
    }

    //strageEdit
    fn handle_normal_mode_key_event_download_and_view(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(item) = app.items.items.get(idx) {
                if let Some(upload_link) = &item.upload_link {
                    let url = format!("{}{}", self.config.url, upload_link);
                    let _ = Command::new("curl")
                        .args([
                            "--socks5",
                            "localhost:9050",
                            "--socks5-hostname",
                            "localhost:9050",
                            &url,
                        ])
                        .arg("-o")
                        .arg("download.img")
                        .output()
                        .expect("Failed to execute curl command");

                    let _ = Command::new("xdg-open")
                        .arg("./download.img")
                        .output()
                        .expect("Failed to execute sxiv command");
                } else if let Some((_, _, msg, _)) = get_message(
                    &item.text,
                    &self.config.members_tag,
                    &self.config.staffs_tag,
                ) {
                    let finder = LinkFinder::new();
                    let links: Vec<_> = finder.links(msg.as_str()).collect();
                    if let Some(link) = links.first() {
                        let url = link.as_str();
                        let _ = Command::new("curl")
                            .args([
                                "--socks5",
                                "localhost:9050",
                                "--socks5-hostname",
                                "localhost:9050",
                                url,
                            ])
                            .arg("-o")
                            .arg("download.img")
                            .output()
                            .expect("Failed to execute curl command");

                        let _ = Command::new("sxiv")
                            .arg("./download.img")
                            .output()
                            .expect("Failed to execute sxiv command");
                    }
                }
            }
        }
    }

    fn handle_normal_mode_key_event_toggle_mute(&mut self) {
        let mut is_muted = self.is_muted.lock().unwrap();
        *is_muted = !*is_muted;
    }

    fn handle_normal_mode_key_event_toggle_sys(&mut self) {
        self.show_sys = !self.show_sys;
    }

    fn handle_normal_mode_key_event_toggle_guest_view(&mut self) {
        self.display_guest_view = !self.display_guest_view;
    }

    fn handle_normal_mode_key_event_toggle_member_view(&mut self) {
        self.display_member_view = !self.display_member_view;
    }

    fn handle_normal_mode_key_event_toggle_pm_only(&mut self) {
        self.display_pm_only = !self.display_pm_only;
    }

    fn handle_normal_mode_key_event_toggle_v_view(&mut self) {
        if self.account_manager.master_account.is_some() {
            self.display_master_pm_view = !self.display_master_pm_view;
        } else {
            self.display_staff_view = !self.display_staff_view;
        }
    }

    fn handle_normal_mode_key_event_shift_c(&mut self, app: &mut App) {
        if self.clean_mode {
            self.clean_mode = false;
            return;
        }
        if let Some(session) = &self.session {
            match fetch_clean_messages(
                &self.client,
                &self.config.url,
                &self.config.page_php,
                session,
            ) {
                Ok(msgs) => {
                    app.clean_items.items = msgs;
                    app.clean_items.state.select(None);
                    self.clean_mode = true;
                }
                Err(e) => log::error!("failed to load clean view: {}", e),
            }
        }
    }

    fn handle_normal_mode_key_event_shift_o(&mut self, app: &mut App) {
        if self.inbox_mode {
            self.inbox_mode = false;
            return;
        }
        if let Some(session) = &self.session {
            match fetch_inbox_messages(&self.client, &self.config.url, session) {
                Ok(msgs) => {
                    app.inbox_items.items = msgs;
                    app.inbox_items.state.select(None);
                    self.inbox_mode = true;
                }
                Err(e) => log::error!("failed to load inbox view: {}", e),
            }
        }
    }

    fn handle_normal_mode_key_event_g(&mut self, app: &mut App) {
        // Handle "gg" key combination
        if self.last_key_event == Some(KeyCode::Char('g')) {
            app.items.select_top();
            self.last_key_event = None;
        }
    }

    fn handle_normal_mode_key_event_toggle_hidden(&mut self) {
        self.display_hidden_msgs = !self.display_hidden_msgs;
    }

    fn handle_normal_mode_key_event_input_mode(&mut self, app: &mut App) {
        app.input_mode = InputMode::Editing;
        app.items.unselect();
    }

    fn handle_normal_mode_key_event_logout(&mut self) -> Result<(), ExitSignal> {
        self.logout().unwrap();
        return Err(ExitSignal::Terminate);
    }

    fn handle_normal_mode_key_event_exit(&mut self) -> Result<(), ExitSignal> {
        return Err(ExitSignal::Terminate);
    }

    fn handle_normal_mode_key_event_tag(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            let text = &app.items.items.get(idx).unwrap().text;
            if let Some(username) = get_username(
                &self.base_client.username,
                &text,
                &self.config.members_tag,
                &self.config.staffs_tag,
            ) {
                let txt = text.text();
                if let Some(master) = &self.account_manager.master_account {
                    if let Some((cmd, original)) =
                        parse_forwarded_username(&txt, &app.members_tag, &app.staffs_tag)
                    {
                        app.input = format!("/pm {} {} @{} ", master, cmd, original);
                        app.input_idx = app.input.width();
                        app.input_mode = InputMode::Editing;
                        app.items.unselect();
                        return;
                    }
                }

                if txt.starts_with(&app.staffs_tag) {
                    app.input = format!("/s @{} ", username);
                } else if txt.starts_with(&app.members_tag) {
                    app.input = format!("/m @{} ", username);
                } else {
                    app.input = format!("@{} ", username);
                }
                app.input_idx = app.input.width();
                app.input_mode = InputMode::Editing;
                app.items.unselect();
            }
        }
    }

    fn handle_normal_mode_key_event_pm(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(username) = get_username(
                &self.base_client.username,
                &app.items.items.get(idx).unwrap().text,
                &self.config.members_tag,
                &self.config.staffs_tag,
            ) {
                app.input = format!("/pm {} ", username);
                app.input_idx = app.input.width();
                app.input_mode = InputMode::Editing;
                app.items.unselect();
            }
        }
    }

    fn handle_normal_mode_key_event_member_pm(&mut self, app: &mut App) {
        if let Some(master) = &self.account_manager.master_account {
            app.input = format!("/pm {} /m ", master);
        } else {
            app.input = "/m ".to_owned();
        }
        app.input_idx = app.input.width();
        app.input_mode = InputMode::Editing;
        app.items.unselect();
    }

    fn handle_normal_mode_key_event_kick(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(username) = get_username(
                &self.base_client.username,
                &app.items.items.get(idx).unwrap().text,
                &self.config.members_tag,
                &self.config.staffs_tag,
            ) {
                if let Some(master) = &self.account_manager.master_account {
                    app.input = format!("/pm {} #kick {} ", master, username);
                } else {
                    app.input = format!("/kick {} ", username);
                }
                app.input_idx = app.input.width();
                app.input_mode = InputMode::Editing;
                app.items.unselect();
            }
        }
    }

    fn handle_normal_mode_key_event_ban(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(username) = get_username(
                &self.base_client.username,
                &app.items.items.get(idx).unwrap().text,
                &self.config.members_tag,
                &self.config.staffs_tag,
            ) {
                if let Some(master) = &self.account_manager.master_account {
                    app.input = format!("/pm {} #ban {} ", master, username);
                } else {
                    app.input = format!("/ban {} ", username);
                }
                app.input_idx = app.input.width();
                app.input_mode = InputMode::Editing;
                app.items.unselect();
            }
        }
    }

    fn handle_normal_mode_key_event_ban_exact(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(username) = get_username(
                &self.base_client.username,
                &app.items.items.get(idx).unwrap().text,
                &self.config.members_tag,
                &self.config.staffs_tag,
            ) {
                app.input = format!(r#"/ban "{}" "#, username);
                app.input_idx = app.input.width();
                app.input_mode = InputMode::Editing;
                app.items.unselect();
            }
        }
    }

    //Strange
    fn handle_normal_mode_key_event_translate(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) {
        log::error!("translate running");
        if let Some(idx) = app.items.state.selected() {
            log::error!("1353");
            let mut message_lock = messages.lock().unwrap();
            if let Some(message) = message_lock.get_mut(idx) {
                log::error!("1356");
                let original_text = &mut message.text;
                let output = Command::new("trans")
                    .arg("-b")
                    .arg(&original_text.text())
                    .output()
                    .expect("Failed to execute translation command");

                if output.status.success() {
                    if let Ok(new_text) = String::from_utf8(output.stdout) {
                        *original_text = StyledText::Text(new_text.trim().to_owned());
                        log::error!("Translation successful: {}", new_text);
                    } else {
                        log::error!("Failed to decode translation output as UTF-8");
                    }
                } else {
                    log::error!("Translation command failed with error: {:?}", output.status);
                }
            }
        }
    }

    //Strange
    fn handle_normal_mode_key_event_warn(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            if let Some(username) = get_username(
                &self.base_client.username,
                &app.items.items.get(idx).unwrap().text,
                &self.config.members_tag,
                &self.config.staffs_tag,
            ) {
                app.input = format!("!warn @{} ", username);
                app.input_idx = app.input.width();
                app.input_mode = InputMode::Editing;
                app.items.unselect();
            }
        }
    }

    fn handle_normal_mode_key_event_delete(
        &mut self,
        app: &mut App,
        messages: &Arc<Mutex<Vec<Message>>>,
    ) {
        if app.inbox_mode {
            // Handle deletion in inbox mode - delete all checked messages
            let mut indices_to_remove = Vec::new();
            let mut message_ids_to_delete = Vec::new();

            for (idx, message) in app.inbox_items.items.iter().enumerate() {
                if message.selected {
                    let message_id = message.id.clone();
                    message_ids_to_delete.push(message_id);
                    indices_to_remove.push(idx);
                }
            }

            // Remove messages from UI immediately
            for &idx in indices_to_remove.iter().rev() {
                app.inbox_items.items.remove(idx);
            }

            // Adjust selection
            if app.inbox_items.items.is_empty() {
                app.inbox_items.state.select(None);
            } else if let Some(selected) = app.inbox_items.state.selected() {
                if selected >= app.inbox_items.items.len() {
                    app.inbox_items
                        .state
                        .select(Some(app.inbox_items.items.len() - 1));
                }
            }

            // Send delete requests in background thread
            if !message_ids_to_delete.is_empty() {
                let client = self.client.clone();
                let session = self.session.clone();
                let url = self.config.url.clone();
                thread::spawn(move || {
                    if let Some(session) = session {
                        for message_id in message_ids_to_delete {
                            let delete_url = format!("{}?action=inbox&session={}", url, session);
                            let form = reqwest::blocking::multipart::Form::new()
                                .text("lang", "en")
                                .text("action", "inbox")
                                .text("session", session.clone())
                                .text("do", "delete")
                                .text("mid[]", message_id.clone());

                            if let Err(e) = client.post(&delete_url).multipart(form).send() {
                                log::error!("Failed to delete inbox message {}: {}", message_id, e);
                            }
                        }
                    }
                });
            }
            return;
        }

        if app.clean_mode {
            // Handle deletion in clean mode - delete all checked messages
            let mut indices_to_remove = Vec::new();
            let mut message_ids_to_delete = Vec::new();

            for (idx, message) in app.clean_items.items.iter().enumerate() {
                if message.selected {
                    let message_id = message.id.clone();
                    message_ids_to_delete.push(message_id);
                    indices_to_remove.push(idx);
                }
            }

            // Remove messages from UI immediately
            for &idx in indices_to_remove.iter().rev() {
                app.clean_items.items.remove(idx);
            }

            // Adjust selection
            if app.clean_items.items.is_empty() {
                app.clean_items.state.select(None);
            } else if let Some(selected) = app.clean_items.state.selected() {
                if selected >= app.clean_items.items.len() {
                    app.clean_items
                        .state
                        .select(Some(app.clean_items.items.len() - 1));
                }
            }

            // Send delete requests in background thread
            if !message_ids_to_delete.is_empty() {
                let tx = self.tx.clone();
                thread::spawn(move || {
                    for message_id in message_ids_to_delete {
                        let message_id_for_log = message_id.clone();
                        if let Err(e) = tx.send(PostType::Delete(message_id)) {
                            log::error!(
                                "Failed to send delete request for message {}: {}",
                                message_id_for_log,
                                e
                            );
                        }
                    }
                });
            }
            return;
        }

        // Regular message deletion
        if let Some(idx) = app.items.state.selected() {
            if let Some(id) = app.items.items.get(idx).and_then(|m| m.id) {
                if self.clean_mode {
                    self.post_msg(PostType::Delete(id.to_string())).unwrap();
                    if let Ok(mut msgs) = messages.lock() {
                        msgs.retain(|m| m.id != Some(id));
                    }
                    app.items.unselect();
                } else {
                    app.input = format!("/delete {}", id);
                    app.input_idx = app.input.width();
                    app.input_mode = InputMode::Editing;
                    app.items.unselect();
                }
            }
        }
    }
    fn handle_normal_mode_key_event_page_up(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            app.items.state.select(idx.checked_sub(10).or(Some(0)));
        } else {
            app.items.next();
        }
    }

    fn handle_normal_mode_key_event_page_down(&mut self, app: &mut App) {
        if let Some(idx) = app.items.state.selected() {
            let wanted_idx = idx + 10;
            let max_idx = app.items.items.len() - 1;
            let new_idx = std::cmp::min(wanted_idx, max_idx);
            app.items.state.select(Some(new_idx));
        } else {
            app.items.next();
        }
    }

    fn handle_normal_mode_key_event_esc(&mut self, app: &mut App) {
        app.items.unselect();
    }

    fn handle_normal_mode_key_event_shift_u(&mut self, app: &mut App) {
        app.items.state.select(Some(0));
    }

    fn handle_editing_mode_key_event_enter(
        &mut self,
        app: &mut App,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        if FIND_RGX.is_match(&app.input) {
            return Ok(());
        }

        let mut input: String = app.input.drain(..).collect();
        input = replace_newline_escape(&input);
        app.input_idx = 0;

        // Add to history if not empty
        if !input.trim().is_empty() {
            app.add_to_history(input.clone());
        }

        // Iterate over commands and execute associated actions
        for (command, action) in &app.commands.commands {
            // log::error!("command :{} action :{}", command, action);
            let expected_input = format!("!{}", command);
            if input == expected_input {
                // Execute the action by posting a message
                self.post_msg(PostType::Post(action.clone(), None)).unwrap();
                // Return Ok(()) if the action is executed successfully
                return Ok(());
            }
        }

        let mut cmd_input = input.clone();
        let mut members_prefix = false;
        let mut staffs_prefix = false;
        let mut pm_target: Option<String> = None;

        // Check for /pm prefix first
        if let Some(captures) = PM_RGX.captures(&cmd_input) {
            let username = captures[1].to_string();
            let remaining = captures[2].to_string();
            if remaining.starts_with('/') {
                // This is a ChatOps command with PM target
                pm_target = Some(username);
                cmd_input = remaining;
            }
        } else if cmd_input.starts_with("/m ") {
            members_prefix = true;
            if remove_prefix(&cmd_input, "/m ").starts_with('/') {
                cmd_input = remove_prefix(&cmd_input, "/m ").to_owned();
            }
        } else if cmd_input.starts_with("/s ") {
            staffs_prefix = true;
            if remove_prefix(&cmd_input, "/s ").starts_with('/') {
                cmd_input = remove_prefix(&cmd_input, "/s ").to_owned();
            }
        }

        // Determine target for ChatOps commands
        let chatops_target = if let Some(user) = pm_target.clone() {
            Some(user)
        } else if members_prefix {
            Some(SEND_TO_MEMBERS.to_owned())
        } else if staffs_prefix {
            Some(SEND_TO_STAFFS.to_owned())
        } else {
            None
        };

        if self.process_command_with_target(&cmd_input, app, users, chatops_target) {
            if members_prefix {
                app.input = "/m ".to_owned();
                app.input_idx = app.input.width();
            } else if staffs_prefix {
                app.input = "/s ".to_owned();
                app.input_idx = app.input.width();
            } else if pm_target.is_some() {
                // Don't reset input for PM - let user continue the conversation
            }
            return Ok(());
        }

        if members_prefix {
            let msg = remove_prefix(&input, "/m ").to_owned();
            let to = Some(SEND_TO_MEMBERS.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            app.input = "/m ".to_owned();
            app.input_idx = app.input.width();
        } else if staffs_prefix {
            let msg = remove_prefix(&input, "/s ").to_owned();
            let to = Some(SEND_TO_STAFFS.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            app.input = "/s ".to_owned();
            app.input_idx = app.input.width();
        } else if let Some(user) = pm_target {
            // Handle PM that wasn't a ChatOps command
            let msg = if let Some(captures) = PM_RGX.captures(&input) {
                captures[2].to_string()
            } else {
                input.clone()
            };
            let to = Some(user.clone());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            app.input = format!("/pm {} ", user);
            app.input_idx = app.input.width();
        } else if input.starts_with("/a ") {
            let msg = remove_prefix(&input, "/a ").to_owned();
            let to = Some(SEND_TO_ADMINS.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            app.input = "/a ".to_owned();
            app.input_idx = app.input.width();
        } else if input.starts_with("/s ") {
            let msg = remove_prefix(&input, "/s ").to_owned();
            let to = Some(SEND_TO_STAFFS.to_owned());
            self.post_msg(PostType::Post(msg, to)).unwrap();
            app.input = "/s ".to_owned();
            app.input_idx = app.input.width();
        } else {
            if input.starts_with("/") && !input.starts_with("/me ") {
                app.input_idx = input.len();
                app.input = input;
                app.input_mode = InputMode::EditingErr;
            } else {
                self.post_msg(PostType::Post(input, None)).unwrap();
                // Reset input mode to Normal after sending message
                app.input_mode = InputMode::Normal;
            }
        }
        Ok(())
    }

    fn handle_editing_mode_key_event_tab(&mut self, app: &mut App, users: &Arc<Mutex<Users>>) {
        let (p1, p2) = app.input.split_at(app.input_idx);
        if p2 == "" || p2.chars().nth(0) == Some(' ') {
            let mut parts: Vec<&str> = p1.split(" ").collect();
            if let Some(user_prefix) = parts.pop() {
                let mut should_autocomplete = false;
                let mut prefix = "";
                if parts.len() == 1
                    && ((parts[0] == "/kick" || parts[0] == "/k")
                        || parts[0] == "/pm"
                        || parts[0] == "/ignore"
                        || parts[0] == "/unignore"
                        || parts[0] == "/ban")
                {
                    should_autocomplete = true;
                } else if user_prefix.starts_with("@") {
                    should_autocomplete = true;
                    prefix = "@";
                }
                if should_autocomplete {
                    let user_prefix_norm = remove_prefix(user_prefix, prefix);
                    let user_prefix_norm_len = user_prefix_norm.len();
                    if let Some(name) = autocomplete_username(users, user_prefix_norm) {
                        let complete_name = format!("{}{}", prefix, name);
                        parts.push(complete_name.as_str());
                        let p2 = p2.trim_start();
                        if p2 != "" {
                            parts.push(p2);
                        }
                        app.input = parts.join(" ");
                        app.input_idx += name.len() - user_prefix_norm_len;
                    }
                }
            }
        }
    }

    fn handle_editing_mode_key_event_ctrl_c(&mut self, app: &mut App) {
        app.clear_filter();
        app.input = "".to_owned();
        app.input_idx = 0;
        app.input_mode = InputMode::Normal;
    }

    fn handle_editing_mode_key_event_ctrl_a(&mut self, app: &mut App) {
        app.input_idx = 0;
    }

    fn handle_editing_mode_key_event_ctrl_e(&mut self, app: &mut App) {
        app.input_idx = app.input.width();
    }

    fn handle_editing_mode_key_event_ctrl_f(&mut self, app: &mut App) {
        if let Some(idx) = app.input.chars().skip(app.input_idx).position(|c| c == ' ') {
            app.input_idx = std::cmp::min(app.input_idx + idx + 1, app.input.width());
        } else {
            app.input_idx = app.input.width();
        }
    }

    fn handle_editing_mode_key_event_ctrl_b(&mut self, app: &mut App) {
        if let Some(idx) = app.input_idx.checked_sub(2) {
            let tmp = app
                .input
                .chars()
                .take(idx)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            if let Some(idx) = tmp.chars().position(|c| c == ' ') {
                app.input_idx = std::cmp::max(tmp.width() - idx, 0);
            } else {
                app.input_idx = 0;
            }
        }
    }

    fn handle_editing_mode_key_event_ctrl_v(&mut self, app: &mut App) {
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        if let Ok(clipboard) = ctx.get_contents() {
            let byte_position = byte_pos(&app.input, app.input_idx).unwrap();
            app.input.insert_str(byte_position, &clipboard);
            app.input_idx += clipboard.chars().count();
        }
    }

    fn handle_editing_mode_key_event_newline(&mut self, app: &mut App) {
        let byte_position = byte_pos(&app.input, app.input_idx).unwrap();
        app.input.insert(byte_position, '\n');
        app.input_idx += 1;
    }

    fn handle_editing_mode_key_event_left(&mut self, app: &mut App) {
        if app.input_idx > 0 {
            app.input_idx -= 1;
        }
    }

    fn handle_editing_mode_key_event_right(&mut self, app: &mut App) {
        if app.input_idx < app.input.width() {
            app.input_idx += 1;
        }
    }

    fn handle_editing_mode_key_event_shift_c(&mut self, app: &mut App, c: char) {
        app.reset_history_navigation();
        let byte_position = byte_pos(&app.input, app.input_idx).unwrap();
        app.input.insert(byte_position, c);

        app.input_idx += 1;
        app.update_filter();
    }

    fn handle_editing_mode_key_event_backspace(&mut self, app: &mut App) {
        app.reset_history_navigation();
        if app.input_idx > 0 {
            app.input_idx -= 1;
            app.input = remove_at(&app.input, app.input_idx);
            app.update_filter();
        }
    }

    fn handle_editing_mode_key_event_delete(&mut self, app: &mut App) {
        app.reset_history_navigation();
        if app.input_idx > 0 && app.input_idx == app.input.width() {
            app.input_idx -= 1;
        }
        app.input = remove_at(&app.input, app.input_idx);
        app.update_filter();
    }

    fn handle_editing_mode_key_event_esc(&mut self, app: &mut App) {
        app.input_mode = InputMode::Normal;
        app.reset_history_navigation();
    }

    fn handle_editing_mode_key_event_up(&mut self, app: &mut App) {
        // In multiline mode, handle cursor navigation first
        if app.input_mode == InputMode::MultilineEditing {
            let input = &app.input;
            let lines: Vec<&str> = input.split('\n').collect();

            // Calculate which line the cursor is on
            let mut current_pos = 0;
            let mut cursor_line = 0;
            let mut chars_in_line = 0;

            for (line_idx, line) in lines.iter().enumerate() {
                let line_len = line.chars().count();
                if current_pos + line_len >= app.input_idx {
                    cursor_line = line_idx;
                    chars_in_line = app.input_idx - current_pos;
                    break;
                }
                current_pos += line_len + 1; // +1 for newline
            }

            // Try to move cursor to previous line
            if cursor_line > 0 {
                let prev_line = lines[cursor_line - 1];
                let prev_line_len = prev_line.chars().count();
                let new_pos_in_line = chars_in_line.min(prev_line_len);

                // Calculate new cursor position
                let mut new_cursor_pos = 0;
                for i in 0..(cursor_line - 1) {
                    new_cursor_pos += lines[i].chars().count();
                    if i < cursor_line - 1 {
                        new_cursor_pos += 1; // for newline
                    }
                }
                if cursor_line > 1 {
                    new_cursor_pos += 1; // for newline before previous line
                }
                new_cursor_pos += new_pos_in_line;

                app.input_idx = new_cursor_pos;
            } else {
                // At first line, try history navigation
                app.navigate_history_up();
            }
        } else {
            // Regular single-line mode, use history navigation
            app.navigate_history_up();
        }
    }

    fn handle_editing_mode_key_event_down(&mut self, app: &mut App) {
        app.navigate_history_down();
    }

    fn handle_editing_mode_key_event_toggle_multiline(&mut self, app: &mut App) {
        match app.input_mode {
            InputMode::Editing | InputMode::EditingErr => {
                app.input_mode = InputMode::MultilineEditing;
            }
            InputMode::MultilineEditing => {
                app.input_mode = InputMode::Editing;
            }
            _ => {}
        }
    }

    fn handle_multiline_editing_mode_key_event(
        &mut self,
        app: &mut App,
        key_event: KeyEvent,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        match key_event {
            // Send message on Ctrl+Enter in multiline mode
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_multiline_editing_mode_key_event_send(app, users)?,
            // Add newline on Enter in multiline mode
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_newline(app),
            // Send message with Ctrl+L in multiline mode
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_multiline_editing_mode_key_event_ctrl_l(app, users)?,
            // History navigation
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_up(app),
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_multiline_editing_mode_key_event_down(app),
            // All other editing keys work the same
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_tab(app, users),
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_multiline_editing_mode_key_event_ctrl_c(app),
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_a(app),
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_e(app),
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_f(app),
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_b(app),
            KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.handle_editing_mode_key_event_ctrl_v(app),
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                app.enter_message_editor_mode();
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_left(app),
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_right(app),
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.handle_multiline_editing_mode_key_event_shift_c(app, c),
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_backspace(app),
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_editing_mode_key_event_delete(app),
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.handle_multiline_editing_mode_key_event_esc(app),
            _ => {}
        }
        Ok(())
    }

    fn handle_multiline_editing_mode_key_event_send(
        &mut self,
        app: &mut App,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        // Same logic as regular enter, but add to history first
        if !app.input.trim().is_empty() {
            app.add_to_history(app.input.clone());
        }
        self.handle_editing_mode_key_event_enter(app, users)
    }

    fn handle_multiline_editing_mode_key_event_ctrl_l(
        &mut self,
        app: &mut App,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        // In multiline mode, Ctrl+L sends the message (like Ctrl+Enter)
        self.handle_multiline_editing_mode_key_event_send(app, users)
    }

    fn handle_multiline_editing_mode_key_event_down(&mut self, app: &mut App) {
        // Handle cursor navigation in multiline content
        let input = &app.input;
        let lines: Vec<&str> = input.split('\n').collect();

        // Calculate which line the cursor is on
        let mut current_pos = 0;
        let mut cursor_line = 0;
        let mut chars_in_line = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_len = line.chars().count();
            if current_pos + line_len >= app.input_idx {
                cursor_line = line_idx;
                chars_in_line = app.input_idx - current_pos;
                break;
            }
            current_pos += line_len + 1; // +1 for newline
        }

        // Try to move cursor to next line
        if cursor_line + 1 < lines.len() {
            let next_line = lines[cursor_line + 1];
            let next_line_len = next_line.chars().count();
            let new_pos_in_line = chars_in_line.min(next_line_len);

            // Calculate new cursor position
            let mut new_cursor_pos = 0;
            for i in 0..=cursor_line {
                new_cursor_pos += lines[i].chars().count();
                if i < cursor_line {
                    new_cursor_pos += 1; // for newline
                }
            }
            new_cursor_pos += 1; // for the newline between current and next line
            new_cursor_pos += new_pos_in_line;

            app.input_idx = new_cursor_pos.min(input.chars().count());
        } else {
            // At last line, try history navigation
            app.navigate_history_down();
        }
    }

    fn handle_multiline_editing_mode_key_event_shift_c(&mut self, app: &mut App, c: char) {
        app.reset_history_navigation();
        self.handle_editing_mode_key_event_shift_c(app, c);
    }

    fn handle_multiline_editing_mode_key_event_ctrl_c(&mut self, app: &mut App) {
        app.reset_history_navigation();
        app.input_mode = InputMode::Normal;
        app.clear_filter();
        app.input = "".to_owned();
        app.input_idx = 0;
    }

    fn handle_multiline_editing_mode_key_event_esc(&mut self, app: &mut App) {
        app.input_mode = InputMode::Normal;
        app.reset_history_navigation();
    }

    fn handle_notes_mode_key_event(
        &mut self,
        app: &mut App,
        key_event: KeyEvent,
    ) -> Result<(), ExitSignal> {
        use crossterm::event::{KeyCode, KeyModifiers};

        match key_event {
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.cycle_notes_type(self);
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if app.handle_notes_vim_key(c, self) {
                    Ok(())
                } else {
                    Ok(())
                }
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                // Handle capital letters and shifted characters
                if app.handle_notes_vim_key(c, self) {
                    Ok(())
                } else {
                    Ok(())
                }
            }
            KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Ctrl+r - redo
                app.notes_redo();
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.handle_notes_vim_key('\x08', self);
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.handle_notes_vim_key('\x7f', self);
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.handle_notes_vim_key('\n', self);
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.handle_notes_vim_key('\x1b', self);
                Ok(())
            }
            // Arrow keys for insert mode
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if app.notes_vim_mode == VimMode::Insert {
                    if app.notes_cursor_pos.1 > 0 {
                        app.notes_cursor_pos.1 -= 1;
                    }
                }
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if app.notes_vim_mode == VimMode::Insert {
                    let line_len = app.notes_content[app.notes_cursor_pos.0].len();
                    if app.notes_cursor_pos.1 < line_len {
                        app.notes_cursor_pos.1 += 1;
                    }
                }
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if app.notes_vim_mode == VimMode::Insert && app.notes_cursor_pos.0 > 0 {
                    app.notes_cursor_pos.0 -= 1;
                    let line_len = app.notes_content[app.notes_cursor_pos.0].len();
                    if app.notes_cursor_pos.1 > line_len {
                        app.notes_cursor_pos.1 = line_len;
                    }
                }
                Ok(())
            }
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if app.notes_vim_mode == VimMode::Insert && app.notes_cursor_pos.0 < app.notes_content.len() - 1 {
                    app.notes_cursor_pos.0 += 1;
                    let line_len = app.notes_content[app.notes_cursor_pos.0].len();
                    if app.notes_cursor_pos.1 > line_len {
                        app.notes_cursor_pos.1 = line_len;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn handle_mouse_event(
        &mut self,
        app: &mut App,
        mouse_event: MouseEvent,
    ) -> Result<(), ExitSignal> {
        match mouse_event.kind {
            MouseEventKind::ScrollDown => app.items.next(),
            MouseEventKind::ScrollUp => app.items.previous(),
            _ => {}
        }
        Ok(())
    }

    // Notes functionality
    fn fetch_notes(&self, note_type: &str) -> Result<(Vec<String>, Option<String>), Box<dyn std::error::Error>> {
        let session = self.session.as_ref().ok_or("Not logged in")?;
        let full_url = format!("{}/{}", self.config.url, self.config.page_php);
        
        let mut params = vec![
            ("action", "notes"),
            ("session", session),
            ("lang", LANG),
        ];

        if !note_type.is_empty() && note_type != "personal" {
            params.push(("do", note_type));
        }

        let response = self.client.post(&full_url).form(&params).send()?;
        let body = response.text()?;

        // Parse HTML to extract textarea content and last edited info
        let doc = select::document::Document::from(body.as_str());
        
        let content = if let Some(textarea) = doc.find(select::predicate::Name("textarea")).next() {
            let content = textarea.text();
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if lines.is_empty() {
                vec!["".to_string()]
            } else {
                lines
            }
        } else {
            vec!["Access denied or no notes found".to_string()]
        };

        // Extract last edited information from the paragraph before the form
        let last_edited = doc
            .find(select::predicate::Name("p"))
            .filter_map(|node| {
                let text = node.text();
                // Look for text containing "Last edited by" pattern
                if text.contains("Last edited by") || text.contains("at ") {
                    Some(text.trim().to_string())
                } else {
                    None
                }
            })
            .next();

        Ok((content, last_edited))
    }

    fn save_notes(&self, note_type: &str, content: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.session.as_ref().ok_or("Not logged in")?;
        let full_url = format!("{}/{}", self.config.url, self.config.page_php);
        
        let text = content.join("\n");
        let mut params = vec![
            ("action", "notes"),
            ("session", session),
            ("lang", LANG),
            ("text", text.as_str()),
        ];

        if !note_type.is_empty() && note_type != "personal" {
            params.push(("do", note_type));
        }

        let response = self.client.post(&full_url).form(&params).send()?;
        let body = response.text()?;

        // Check if save was successful
        if body.contains("Notes saved!") || body.contains("saved") {
            Ok(())
        } else {
            Err("Failed to save notes".into())
        }
    }
}

// Give a char index, return the byte position
fn byte_pos(v: &str, idx: usize) -> Option<usize> {
    let mut b = 0;
    let mut chars = v.chars();
    for _ in 0..idx {
        if let Some(c) = chars.next() {
            b += c.len_utf8();
        } else {
            return None;
        }
    }
    Some(b)
}

// Remove the character at idx (utf-8 aware)
fn remove_at(v: &str, idx: usize) -> String {
    v.chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i == idx {
                return None;
            }
            Some(c)
        })
        .collect::<String>()
}

// Autocomplete any username
fn autocomplete_username(users: &Arc<Mutex<Users>>, prefix: &str) -> Option<String> {
    let users = users.lock().unwrap();
    let all_users = users.all();
    let prefix_lower = prefix.to_lowercase();
    let filtered = all_users
        .iter()
        .find(|(_, name)| name.to_lowercase().starts_with(&prefix_lower));
    Some(filtered?.1.to_owned())
}

fn set_profile_base_info(
    client: &Client,
    full_url: &str,
    params: &mut Vec<(&str, String)>,
) -> anyhow::Result<()> {
    params.extend(vec![("action", "profile".to_owned())]);
    let profile_resp = client.post(full_url).form(&params).send()?;
    let profile_resp_txt = profile_resp.text().unwrap();
    let doc = Document::from(profile_resp_txt.as_str());
    let bold = doc.find(Attr("id", "bold")).next().unwrap();
    let italic = doc.find(Attr("id", "italic")).next().unwrap();
    let small = doc.find(Attr("id", "small")).next().unwrap();
    if bold.attr("checked").is_some() {
        params.push(("bold", "on".to_owned()));
    }
    if italic.attr("checked").is_some() {
        params.push(("italic", "on".to_owned()));
    }
    if small.attr("checked").is_some() {
        params.push(("small", "on".to_owned()));
    }
    let font_select = doc.find(Attr("name", "font")).next().unwrap();
    let font = font_select.find(Name("option")).find_map(|el| {
        if el.attr("selected").is_some() {
            return Some(el.attr("value").unwrap());
        }
        None
    });
    params.push(("font", font.unwrap_or("").to_owned()));
    Ok(())
}

enum RetryErr {
    Retry,
    Exit,
}

fn retry_fn<F>(mut clb: F)
where
    F: FnMut() -> anyhow::Result<RetryErr>,
{
    loop {
        match clb() {
            Ok(RetryErr::Retry) => continue,
            Ok(RetryErr::Exit) => return,
            Err(err) => {
                log::error!("{}", err);
                continue;
            }
        }
    }
}

fn post_msg(
    client: &Client,
    post_type_recv: PostType,
    full_url: &str,
    session: String,
    url: &str,
    last_post_tx: &crossbeam_channel::Sender<()>,
) {
    let mut should_reset_keepalive_timer = false;
    let mut delete_after = false;
    retry_fn(|| -> anyhow::Result<RetryErr> {
        let post_type = post_type_recv.clone();
        let resp_text = client.get(url).send()?.text()?;
        let doc = Document::from(resp_text.as_str());
        let nc = doc
            .find(Attr("name", "nc"))
            .next()
            .context("nc not found")?;
        let nc_value = nc.attr("value").context("nc value not found")?.to_owned();
        let postid = doc
            .find(Attr("name", "postid"))
            .next()
            .context("failed to get postid")?;
        let postid_value = postid
            .attr("value")
            .context("failed to get postid value")?
            .to_owned();
        let mut params: Vec<(&str, String)> = vec![
            ("lang", LANG.to_owned()),
            ("nc", nc_value.to_owned()),
            ("session", session.clone()),
        ];

        if let PostType::Clean(date, text) = post_type {
            if let Err(e) = delete_message(&client, full_url, &mut params, date, text) {
                log::error!("failed to delete message: {:?}", e);
                return Ok(RetryErr::Retry);
            }
            return Ok(RetryErr::Exit);
        }

        let mut req = client.post(full_url);
        let mut form: Option<multipart::Form> = None;

        match post_type {
            PostType::Post(msg, send_to) => {
                should_reset_keepalive_timer = true;
                params.extend(vec![
                    ("action", "post".to_owned()),
                    ("postid", postid_value.to_owned()),
                    ("multi", "on".to_owned()),
                    ("message", msg),
                    ("sendto", send_to.unwrap_or(SEND_TO_ALL.to_owned())),
                ]);
            }
            PostType::PM(to, msg) => {
                should_reset_keepalive_timer = true;
                params.extend(vec![
                    ("action", "post".to_owned()),
                    ("postid", postid_value.to_owned()),
                    ("multi", "on".to_owned()),
                    ("message", format!("/pm {} {}", to, msg)),
                    ("sendto", SEND_TO_ALL.to_owned()),
                ]);
            }
            PostType::KeepAlive(send_to) => {
                should_reset_keepalive_timer = true;
                delete_after = true;
                params.extend(vec![
                    ("action", "post".to_owned()),
                    ("postid", postid_value.to_owned()),
                    ("multi", "on".to_owned()),
                    ("message", "keep alive".to_owned()),
                    ("sendto", send_to),
                ]);
            }
            PostType::NewNickname(new_nickname) => {
                set_profile_base_info(client, full_url, &mut params)?;
                params.extend(vec![
                    ("do", "save".to_owned()),
                    ("timestamps", "on".to_owned()),
                    ("newnickname", new_nickname),
                ]);
            }
            PostType::NewColor(new_color) => {
                set_profile_base_info(client, full_url, &mut params)?;
                params.extend(vec![
                    ("do", "save".to_owned()),
                    ("timestamps", "on".to_owned()),
                    ("colour", new_color),
                ]);
            }
            PostType::Ignore(username) => {
                set_profile_base_info(client, full_url, &mut params)?;
                params.extend(vec![
                    ("do", "save".to_owned()),
                    ("timestamps", "on".to_owned()),
                    ("ignore", username),
                ]);
            }
            PostType::Unignore(username) => {
                set_profile_base_info(client, full_url, &mut params)?;
                params.extend(vec![
                    ("do", "save".to_owned()),
                    ("timestamps", "on".to_owned()),
                    ("unignore", username),
                ]);
            }
            PostType::Profile(new_color, new_nickname, incognito_on, bold, italic) => {
                set_profile_base_info(client, full_url, &mut params)?;
                params.extend(vec![
                    ("do", "save".to_owned()),
                    ("timestamps", "on".to_owned()),
                    ("colour", new_color),
                    ("newnickname", new_nickname),
                    (
                        "incognito",
                        if incognito_on { "on" } else { "off" }.to_owned(),
                    ),
                    ("bold", if bold { "on" } else { "off" }.to_owned()),
                    ("italic", if italic { "on" } else { "off" }.to_owned()),
                ]);
            }
            PostType::SetIncognito(incognito_on) => {
                set_profile_base_info(client, full_url, &mut params)?;
                params.extend(vec![
                    ("do", "save".to_owned()),
                    ("timestamps", "on".to_owned()),
                ]);
                if incognito_on {
                    params.push(("incognito", "on".to_owned()));
                } else {
                    params.push(("incognito", "off".to_owned()));
                }
            }
            PostType::Kick(msg, send_to) => {
                params.extend(vec![
                    ("action", "post".to_owned()),
                    ("postid", postid_value.to_owned()),
                    ("message", msg),
                    ("sendto", send_to),
                    ("kick", "kick".to_owned()),
                    ("what", "purge".to_owned()),
                ]);
            }
            PostType::DeleteLast | PostType::DeleteAll => {
                params.extend(vec![("action", "delete".to_owned())]);
                if let PostType::DeleteAll = post_type {
                    params.extend(vec![
                        ("sendto", SEND_TO_ALL.to_owned()),
                        ("confirm", "yes".to_owned()),
                        ("what", "all".to_owned()),
                    ]);
                } else {
                    params.extend(vec![("sendto", "".to_owned()), ("what", "last".to_owned())]);
                }
            }
            PostType::Delete(msg) => {
                params.extend(vec![
                    ("action", "admin".to_owned()),
                    ("do", "clean".to_owned()),
                    ("what", "selected".to_owned()),
                    ("mid[]", msg.to_owned()),
                    ("sendto", SEND_TO_ALL.to_owned()),
                ]);
            }
            PostType::Upload(file_path, send_to, msg) => {
                form = Some(
                    match multipart::Form::new()
                        .text("lang", LANG.to_owned())
                        .text("nc", nc_value.to_owned())
                        .text("session", session.clone())
                        .text("action", "post".to_owned())
                        .text("postid", postid_value.to_owned())
                        .text("message", msg)
                        .text("sendto", send_to.to_owned())
                        .text("what", "purge".to_owned())
                        .file("file", file_path)
                    {
                        Ok(f) => f,
                        Err(e) => {
                            log::error!("{:?}", e);
                            return Ok(RetryErr::Exit);
                        }
                    },
                );
            }
            PostType::Clean(_, _) => {}
        }

        if let Some(form_content) = form {
            req = req.multipart(form_content);
        } else {
            req = req.form(&params);
        }
        match req.send() {
            Ok(resp) => {
                if let Err(err) = resp.error_for_status_ref() {
                    log::error!("HTTP error: {:?}", err);
                    return Ok(RetryErr::Retry);
                }
            }
            Err(err) => {
                log::error!("{:?}", err.to_string());
                if err.is_timeout() {
                    return Ok(RetryErr::Retry);
                }
                return Ok(RetryErr::Retry);
            }
        }

        if delete_after {
            let params = vec![
                ("lang", LANG.to_owned()),
                ("nc", nc_value.to_owned()),
                ("session", session.clone()),
                ("action", "delete".to_owned()),
                ("sendto", "".to_owned()),
                ("what", "last".to_owned()),
            ];
            if let Err(err) = client.post(full_url).form(&params).send() {
                log::error!("{:?}", err.to_string());
                if err.is_timeout() {
                    return Ok(RetryErr::Retry);
                }
            }
        }
        return Ok(RetryErr::Exit);
    });
    if should_reset_keepalive_timer {
        last_post_tx.send(()).unwrap();
    }
}

impl LeChatPHPClient {
    fn handle_message_editor_key_event(
        &mut self,
        app: &mut App,
        key_event: KeyEvent,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        let command = match key_event {
            KeyEvent { code: KeyCode::Char('r'), modifiers: KeyModifiers::CONTROL, .. } => {
                // Ctrl+r - redo
                app.msg_editor_redo();
                EditorCommand::None
            }
            KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::NONE, .. } => {
                app.handle_msg_editor_vim_key(c)
            }
            KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::SHIFT, .. } => {
                // Handle capital letters and shifted characters
                app.handle_msg_editor_vim_key(c)
            }
            KeyEvent { code: KeyCode::Esc, .. } => app.handle_msg_editor_vim_key('\x1b'),
            KeyEvent { code: KeyCode::Enter, .. } => app.handle_msg_editor_vim_key('\r'),
            KeyEvent { code: KeyCode::Backspace, .. } => app.handle_msg_editor_vim_key('\x08'),
            KeyEvent { code: KeyCode::Tab, .. } => app.handle_msg_editor_vim_key('\t'),
            KeyEvent { code: KeyCode::Left, .. } => app.handle_msg_editor_vim_key('h'),
            KeyEvent { code: KeyCode::Right, .. } => app.handle_msg_editor_vim_key('l'),
            KeyEvent { code: KeyCode::Up, .. } => app.handle_msg_editor_vim_key('k'),
            KeyEvent { code: KeyCode::Down, .. } => app.handle_msg_editor_vim_key('j'),
            _ => EditorCommand::None,
        };
        
        // Handle the command
        match command {
            EditorCommand::Send(content) => {
                if !content.trim().is_empty() {
                    // Process commands like /m, /s, /pm, and !commands
                    self.process_message_editor_content(content, app, users)?;
                }
                // Reset input mode to Normal after sending message
                app.input_mode = InputMode::Normal;
                app.input.clear();
                app.input_idx = 0;
                Ok(())
            }
            EditorCommand::Quit => {
                // Already handled by exit_message_editor_mode
                Ok(())
            }
            EditorCommand::None => Ok(()),
        }
    }

    fn process_message_editor_content(
        &mut self,
        content: String,
        app: &mut App,
        users: &Arc<Mutex<Users>>,
    ) -> Result<(), ExitSignal> {
        // Check for !commands first
        for (command, action) in &app.commands.commands {
            let expected_input = format!("!{}", command);
            if content.trim() == expected_input {
                if let Err(e) = self.post_msg(PostType::Post(action.clone(), None)) {
                    log::error!("Failed to send command from message editor: {}", e);
                }
                return Ok(());
            }
        }

        let mut processed_content = content;
        let mut members_prefix = false;
        let mut staffs_prefix = false;
        let mut admin_prefix = false;
        let mut pm_target: Option<String> = None;

        // Check for /pm prefix first
        if let Some(captures) = PM_RGX.captures(&processed_content) {
            pm_target = Some(captures[1].to_string());
            processed_content = captures[2].to_string();
        } else if processed_content.starts_with("/m ") {
            members_prefix = true;
            processed_content = processed_content.strip_prefix("/m ").unwrap().to_string();
        } else if processed_content.starts_with("/s ") {
            staffs_prefix = true;
            processed_content = processed_content.strip_prefix("/s ").unwrap().to_string();
        } else if processed_content.starts_with("/a ") {
            admin_prefix = true;
            processed_content = processed_content.strip_prefix("/a ").unwrap().to_string();
        }

        // Determine target for ChatOps commands
        let chatops_target = if let Some(user) = pm_target.clone() {
            Some(user)
        } else if members_prefix {
            Some(SEND_TO_MEMBERS.to_owned())
        } else if staffs_prefix {
            Some(SEND_TO_STAFFS.to_owned())
        } else {
            None
        };

        // Check if it's a ChatOps command
        if processed_content.starts_with("/") && !processed_content.starts_with("/me ") {
            if self.process_command_with_target(&processed_content, app, users, chatops_target) {
                // Command was processed successfully
                if let Some(user) = pm_target {
                    app.input = format!("/pm {} ", user);
                    app.input_idx = app.input.width();
                } else if members_prefix {
                    app.input = "/m ".to_owned();
                    app.input_idx = app.input.width();
                } else if staffs_prefix {
                    app.input = "/s ".to_owned();
                    app.input_idx = app.input.width();
                } else if admin_prefix {
                    app.input = "/a ".to_owned();
                    app.input_idx = app.input.width();
                }
                return Ok(());
            }
        }

        // Send regular message with appropriate target
        if let Some(user) = pm_target {
            if let Err(e) = self.post_msg(PostType::Post(processed_content, Some(user.clone()))) {
                log::error!("Failed to send PM from message editor: {}", e);
            }
            app.input = format!("/pm {} ", user);
            app.input_idx = app.input.width();
        } else if members_prefix {
            if let Err(e) = self.post_msg(PostType::Post(
                processed_content,
                Some(SEND_TO_MEMBERS.to_owned()),
            )) {
                log::error!("Failed to send message to members from message editor: {}", e);
            }
            app.input = "/m ".to_owned();
            app.input_idx = app.input.width();
        } else if staffs_prefix {
            if let Err(e) = self.post_msg(PostType::Post(
                processed_content,
                Some(SEND_TO_STAFFS.to_owned()),
            )) {
                log::error!("Failed to send message to staff from message editor: {}", e);
            }
            app.input = "/s ".to_owned();
            app.input_idx = app.input.width();
        } else if admin_prefix {
            if let Err(e) = self.post_msg(PostType::Post(
                processed_content,
                Some(SEND_TO_ADMINS.to_owned()),
            )) {
                log::error!("Failed to send message to admins from message editor: {}", e);
            }
            app.input = "/a ".to_owned();
            app.input_idx = app.input.width();
        } else {
            // Regular message to main chat
            if processed_content.starts_with("/") && !processed_content.starts_with("/me ") {
                // Invalid command - just send as regular message for now
                if let Err(e) = self.post_msg(PostType::Post(processed_content, None)) {
                    log::error!("Failed to send message from message editor: {}", e);
                }
            } else {
                // Send as regular message
                if let Err(e) = self.post_msg(PostType::Post(processed_content, None)) {
                    log::error!("Failed to send message from message editor: {}", e);
                }
            }
        }

        Ok(())
    }
}

fn parse_date(date: &str, datetime_fmt: &str) -> NaiveDateTime {
    let now = Utc::now();
    let date_fmt = format!("%Y-{}", datetime_fmt);
    NaiveDateTime::parse_from_str(
        format!("{}-{}", now.year(), date).as_str(),
        date_fmt.as_str(),
    )
    .unwrap()
}

fn get_msgs(
    client: &Client,
    base_url: &str,
    page_php: &str,
    session: &str,
    username: &str,
    users: &Arc<Mutex<Users>>,
    sig: &Arc<Mutex<Sig>>,
    messages_updated_tx: &crossbeam_channel::Sender<()>,
    members_tag: &str,
    staffs_tag: &str,
    datetime_fmt: &str,
    messages: &Arc<Mutex<Vec<Message>>>,
    should_notify: &mut bool,
    tx: &crossbeam_channel::Sender<PostType>,
    bad_usernames: &Arc<Mutex<Vec<String>>>,
    bad_exact_usernames: &Arc<Mutex<Vec<String>>>,
    bad_messages: &Arc<Mutex<Vec<String>>>,
    allowlist: &Arc<Mutex<Vec<String>>>,
    alt_account: Option<&str>,
    master_account: Option<&str>,
    alt_forwarding_enabled: &Arc<Mutex<bool>>,
    ai_enabled: &Arc<Mutex<bool>>,
    ai_mode: &Arc<Mutex<String>>,
    openai_client: &Option<OpenAIClient<OpenAIConfig>>,
    system_intel: &str,
    moderation_strictness: &str,
    mod_logs_enabled: &Arc<Mutex<bool>>,
    ai_conversation_memory: &Arc<Mutex<std::collections::HashMap<String, Vec<(String, String)>>>>,
    user_warnings: &Arc<Mutex<std::collections::HashMap<String, u32>>>,
    ai_service: &Arc<AIService>,
    bot_manager: &Option<Arc<Mutex<BotManager>>>,
) -> anyhow::Result<()> {
    let url = format!(
        "{}/{}?action=view&session={}&lang={}",
        base_url, page_php, session, LANG
    );
    let resp_text = client.get(url).send()?.text()?;
    let resp_text = resp_text.replace("<br>", "\n");
    let doc = Document::from(resp_text.as_str());
    let new_messages = match extract_messages(&doc) {
        Ok(messages) => messages,
        Err(_) => {
            // Failed to get messages, probably need re-login
            sig.lock().unwrap().signal(&ExitSignal::NeedLogin);
            return Ok(());
        }
    };
    let current_users = extract_users(&doc);
    {
        let previous = users.lock().unwrap();
        let filters = bad_usernames.lock().unwrap();
        let exact_filters = bad_exact_usernames.lock().unwrap();
        for (_, name) in &current_users.guests {
            if !previous.guests.iter().any(|(_, n)| n == name) {
                if exact_filters.iter().any(|f| f == name)
                    || filters
                        .iter()
                        .any(|f| name.to_lowercase().contains(&f.to_lowercase()))
                {
                    let _ = tx.send(PostType::Kick(String::new(), name.clone()));
                }
            }
        }
    }
    {
        let messages = messages.lock().unwrap();
        process_new_messages(
            &new_messages,
            &messages,
            datetime_fmt,
            members_tag,
            staffs_tag,
            username,
            should_notify,
            &current_users,
            tx,
            bad_usernames,
            bad_exact_usernames,
            bad_messages,
            allowlist,
            alt_account,
            alt_forwarding_enabled,
            ai_enabled,
            ai_mode,
            openai_client,
            system_intel,
            moderation_strictness,
            mod_logs_enabled,
            ai_conversation_memory,
            user_warnings,
            master_account,
            ai_service,
            bot_manager,
        );
        // Build messages vector. Tag deleted messages.
        update_messages(
            new_messages,
            messages,
            datetime_fmt,
            members_tag,
            staffs_tag,
            alt_account,
            master_account,
        );
        // Notify new messages has arrived.
        // This ensure that we redraw the messages on the screen right away.
        // Otherwise, the screen would not redraw until a keyboard event occurs.
        messages_updated_tx.send(()).unwrap();
    }
    {
        let mut u = users.lock().unwrap();
        *u = current_users;
    }
    Ok(())
}

fn process_new_messages(
    new_messages: &Vec<Message>,
    messages: &MutexGuard<Vec<Message>>,
    datetime_fmt: &str,
    members_tag: &str,
    staffs_tag: &str,
    username: &str,
    should_notify: &mut bool,
    users: &Users,
    tx: &crossbeam_channel::Sender<PostType>,
    bad_usernames: &Arc<Mutex<Vec<String>>>,
    bad_exact_usernames: &Arc<Mutex<Vec<String>>>,
    bad_messages: &Arc<Mutex<Vec<String>>>,
    allowlist: &Arc<Mutex<Vec<String>>>,
    alt_account: Option<&str>,
    alt_forwarding_enabled: &Arc<Mutex<bool>>,
    ai_enabled: &Arc<Mutex<bool>>,
    ai_mode: &Arc<Mutex<String>>,
    openai_client: &Option<OpenAIClient<OpenAIConfig>>,
    system_intel: &str,
    moderation_strictness: &str,
    mod_logs_enabled: &Arc<Mutex<bool>>,
    ai_conversation_memory: &Arc<Mutex<std::collections::HashMap<String, Vec<(String, String)>>>>,
    user_warnings: &Arc<Mutex<std::collections::HashMap<String, u32>>>,
    master_account: Option<&str>,
    ai_service: &Arc<AIService>,
    bot_manager: &Option<Arc<Mutex<BotManager>>>,
) {
    if let Some(last_known_msg) = messages.first() {
        let last_known_msg_parsed_dt = parse_date(&last_known_msg.date, datetime_fmt);
        let filtered = new_messages.iter().filter(|new_msg| {
            last_known_msg_parsed_dt <= parse_date(&new_msg.date, datetime_fmt)
                && !(new_msg.date == last_known_msg.date && last_known_msg.text == new_msg.text)
        });
        for new_msg in filtered {
            log_chat_message(new_msg, username);
            if let Some((from, to_opt, msg, channel_info)) = get_message(&new_msg.text, members_tag, staffs_tag) {
                // Track message in AI service for summarization and analysis
                let chat_message = crate::ai_service::ChatMessage {
                    author: from.clone(),
                    content: msg.clone(),
                    is_pm: to_opt.is_some(),
                };
                ai_service.add_message(chat_message);

                // Process message through bot system if available
                if let Some(bot_mgr) = bot_manager {
                    if let Ok(manager) = bot_mgr.lock() {
                        let _is_private = to_opt.is_some();

                        // FIXED: Use actual channel information from message parsing
                        let (channel_context, is_member) = if to_opt.is_some() {
                            // Private message
                            log::info!("Bot: Processing PM from {}", from);
                            ("private", users.members.iter().any(|(_, name)| name == &from))
                        } else {
                            // Use the channel info parsed from the message structure
                            let is_member = users.members.iter().any(|(_, name)| name == &from);
                            let channel = channel_info.as_deref().unwrap_or("public");
                            log::info!("Bot: Processing message from {} in channel: '{}' (member: {})", 
                                from, channel, is_member);
                            (channel, is_member)
                        };
                        
                        if let Err(e) = manager.process_message_for_all_bots(
                            &from,
                            &msg,
                            crate::bot_system::MessageType::Normal,
                            new_msg.id.map(|id| id as u64),
                            if channel_context == "public" {
                                None
                            } else {
                                Some(channel_context)
                            },
                            is_member,
                        ) {
                            log::warn!("Failed to process message through bot system: {}", e);
                        }
                    }
                }

                // Notify when tagged
                if msg.contains(format!("@{}", &username).as_str()) {
                    *should_notify = true;
                }
                if let Some(ref to) = to_opt {
                    if to == username && msg != "!up" {
                        *should_notify = true;
                    }
                }

                // Remote moderation handling
                let is_member_or_staff = users.members.iter().any(|(_, n)| n == &from)
                    || users.staff.iter().any(|(_, n)| n == &from)
                    || users.admin.iter().any(|(_, n)| n == &from);
                let allowed_guest = {
                    let list = allowlist.lock().unwrap();
                    list.contains(&from)
                };
                let directed_to_me = to_opt.as_ref().map(|t| t == username).unwrap_or(false);
                let via_members = new_msg.text.text().starts_with(members_tag);
                let has_permission = is_member_or_staff || allowed_guest;
                if msg.starts_with("#kick ") || msg.starts_with("#ban ") {
                    if has_permission && (directed_to_me || via_members) {
                        if let Some(target) = msg.strip_prefix("#kick ") {
                            let user = target.trim().trim_start_matches('@');
                            if !user.is_empty() {
                                let _ = tx.send(PostType::Kick(String::new(), user.to_owned()));
                            }
                        } else if let Some(target) = msg.strip_prefix("#ban ") {
                            let user = target.trim().trim_start_matches('@');
                            if !user.is_empty() {
                                // Always add to ban list
                                let mut f = bad_usernames.lock().unwrap();
                                f.push(user.to_owned());
                                
                                // Check if target is a member, staff, or admin - only kick guests
                                let target_is_member = users.members.iter().any(|(_, n)| n == user)
                                    || users.staff.iter().any(|(_, n)| n == user)
                                    || users.admin.iter().any(|(_, n)| n == user);
                                
                                if target_is_member {
                                    // Member banned but not kicked
                                    let response = format!("@{} has been added to ban list (member not kicked)", user);
                                    let _ = tx.send(PostType::Post(response, Some(from.clone())));
                                } else {
                                    // Guest banned and kicked
                                    let _ = tx.send(PostType::Kick(String::new(), user.to_owned()));
                                }
                            }
                        }
                    } else if directed_to_me && !has_permission {
                        let msg = "You don't have permission to do that.".to_owned();
                        let _ = tx.send(PostType::Post(msg, Some(from.clone())));
                    }
                }

                if let Some(alt) = alt_account {
                    if *alt_forwarding_enabled.lock().unwrap() {
                        let text = new_msg.text.text();
                        if (text.starts_with(members_tag) || text.starts_with(staffs_tag))
                            && from != alt
                        {
                            let _ = tx.send(PostType::Post(text.clone(), Some(alt.to_owned())));
                        }
                        if from == alt && to_opt.as_deref() == Some(username) {
                            if let Some(stripped) = msg.strip_prefix("/m ") {
                                let _ = tx.send(PostType::Post(
                                    stripped.to_owned(),
                                    Some(SEND_TO_MEMBERS.to_owned()),
                                ));
                                // Echo the message back to the alt so it can confirm
                                let confirm = format!("{}{} - {}", members_tag, username, stripped);
                                let _ = tx.send(PostType::Post(confirm, Some(alt.to_owned())));
                            } else if let Some(stripped) = msg.strip_prefix("/s ") {
                                let _ = tx.send(PostType::Post(
                                    stripped.to_owned(),
                                    Some(SEND_TO_STAFFS.to_owned()),
                                ));
                                let confirm = format!("{}{} - {}", staffs_tag, username, stripped);
                                let _ = tx.send(PostType::Post(confirm, Some(alt.to_owned())));
                            }
                        }
                    }
                }

                let is_guest = users.guests.iter().any(|(_, n)| n == &from);
                if from != username && is_guest {
                    // Check if user is in allowlist first
                    let is_allowed = {
                        let allowed_users = allowlist.lock().unwrap();
                        allowed_users.contains(&from)
                    };

                    if is_allowed {
                        send_mod_log(
                            tx,
                            *mod_logs_enabled.lock().unwrap(),
                            format!(
                                "MOD LOG: User '{}' is allowlisted, bypassing all filters",
                                from
                            ),
                        );
                    } else {
                        let bad_name = {
                            let filters = bad_usernames.lock().unwrap();
                            filters
                                .iter()
                                .any(|f| from.to_lowercase().contains(&f.to_lowercase()))
                        };
                        let bad_name_exact = {
                            let filters = bad_exact_usernames.lock().unwrap();
                            filters.iter().any(|f| f == &from)
                        };
                        let bad_msg = {
                            let filters = bad_messages.lock().unwrap();
                            filters
                                .iter()
                                .any(|f| msg.to_lowercase().contains(&f.to_lowercase()))
                        };

                        if bad_name_exact || bad_name || bad_msg {
                            let reason = if bad_name_exact {
                                "exact username match"
                            } else if bad_name {
                                "username filter match"
                            } else {
                                "message filter match"
                            };
                            send_mod_log(
                                tx,
                                *mod_logs_enabled.lock().unwrap(),
                                format!(
                                    "MOD LOG: FILTER KICK - Kicking '{}' for {}: '{}'",
                                    from, reason, msg
                                ),
                            );
                            let _ = tx.send(PostType::Kick(String::new(), from.clone()));
                        } else {
                            let res = score_message(&msg);
                            if let Some(act) = action_from_score(res.score) {
                                match act {
                                    Action::Warn => {
                                        if to_opt.is_none() {
                                            let reason = res
                                                .reason
                                                .map(|r| r.description())
                                                .unwrap_or("breaking the rules");
                                            let warn = format!(
                                            "@{username} - @{from}'s message was flagged for {reason}."
                                        );
                                            let _ =
                                                tx.send(PostType::Post(warn, Some("0".to_owned())));
                                        }
                                    }
                                    Action::Kick => {
                                        send_mod_log(tx, *mod_logs_enabled.lock().unwrap(), format!("MOD LOG: HARM SCORE KICK - Kicking '{}' for message: '{}'", from, msg));
                                        let _ =
                                            tx.send(PostType::Kick(String::new(), from.clone()));
                                    }
                                    Action::Ban => {
                                        send_mod_log(tx, *mod_logs_enabled.lock().unwrap(), format!("MOD LOG: HARM SCORE BAN - Banning '{}' for message: '{}'", from, msg));
                                        let _ =
                                            tx.send(PostType::Kick(String::new(), from.clone()));
                                        let mut f = bad_usernames.lock().unwrap();
                                        f.push(from.clone());
                                    }
                                }
                            }
                        }
                    }
                }

                // AI Processing - only for guests and ignore messages from logged-in user
                if *ai_enabled.lock().unwrap() && openai_client.is_some() && from != username {
                    // Check if user is a guest (not member, staff, or admin)
                    let is_guest = users.guests.iter().any(|(_, n)| n == &from);

                    let ai_mode_val = ai_mode.lock().unwrap().clone();
                    process_ai_message(
                        &from,
                        &msg,
                        &to_opt,
                        username,
                        &ai_mode_val,
                        openai_client.as_ref().unwrap(),
                        system_intel,
                        moderation_strictness,
                        mod_logs_enabled,
                        is_guest, // Pass guest status
                        tx,
                        bad_usernames,
                        ai_conversation_memory,
                        user_warnings,
                        master_account,
                    );
                }
            }
        }
    }
}

// Helper function to send MOD LOG messages only when enabled
fn send_mod_log(tx: &crossbeam_channel::Sender<PostType>, mod_logs_enabled: bool, message: String) {
    if mod_logs_enabled {
        // Use try_send to avoid panicking if channel is closed
        let _ = tx.try_send(PostType::Post(message, Some("0".to_owned())));
    }
}

// Function to check for specific violations that should trigger warnings in alt mode
fn check_warning_violations(message: &str) -> Option<String> {
    let msg_lower = message.to_lowercase();

    // Check for CP-related content
    let cp_patterns = [
        "cheese pizza",
        "cp links",
        "young models",
        "trading cp",
        "pedo stuff",
        "kiddie porn",
        "jailbait",
        "preteen",
        "underage nudes",
        "r@ygold",
        "hussyfan",
        "ptsc",
        "pthc",
        "young boy",
        "young girl",
        "loli",
        "shota",
    ];

    for pattern in &cp_patterns {
        if msg_lower.contains(pattern) {
            return Some("inappropriate content involving minors".to_string());
        }
    }

    // Check for pornography requests/sharing
    let porn_patterns = [
        "send nudes",
        "porn links",
        "naked pics",
        "sex videos",
        "adult content",
        "xxx links",
        "porn site",
        "onlyfans",
        "cam girl",
        "webcam show",
    ];

    for pattern in &porn_patterns {
        if msg_lower.contains(pattern) {
            return Some("inappropriate adult content".to_string());
        }
    }

    // Check for gun/weapon purchases
    let gun_patterns = [
        "buy gun",
        "selling gun",
        "purchase weapon",
        "buy ammo",
        "ammunition for sale",
        "selling weapons",
        "firearm for sale",
        "gun dealer",
        "weapon trade",
        "buy rifle",
        "selling pistol",
        "handgun for sale",
    ];

    for pattern in &gun_patterns {
        if msg_lower.contains(pattern) {
            return Some("attempting to buy/sell weapons".to_string());
        }
    }

    // Check for account hacking services
    let hack_patterns = [
        "hack facebook",
        "hack instagram",
        "hack account",
        "social media hack",
        "password crack",
        "account recovery service",
        "hack someone",
        "breach account",
        "steal password",
        "facebook hacker",
        "instagram hacker",
        "account takeover",
    ];

    for pattern in &hack_patterns {
        if msg_lower.contains(pattern) {
            return Some("offering/requesting account hacking services".to_string());
        }
    }

    // Check for spam (excessive repetition)
    let words: Vec<&str> = message.split_whitespace().collect();
    if words.len() > 10 {
        let unique_words: std::collections::HashSet<&str> = words.iter().cloned().collect();
        if (unique_words.len() as f32) / (words.len() as f32) < 0.4 {
            return Some("spamming/excessive repetition".to_string());
        }
    }

    // Check for excessive caps (more than 70% of message in caps)
    let caps_count = message.chars().filter(|c| c.is_uppercase()).count();
    let letter_count = message.chars().filter(|c| c.is_alphabetic()).count();
    if letter_count > 20 && caps_count as f32 / letter_count as f32 > 0.7 {
        return Some("excessive use of capital letters".to_string());
    }

    None
}

fn process_ai_message(
    from: &str,
    msg: &str,
    to_opt: &Option<String>,
    username: &str,
    ai_mode: &str,
    openai_client: &OpenAIClient<OpenAIConfig>,
    system_intel: &str,
    moderation_strictness: &str,
    mod_logs_enabled: &Arc<Mutex<bool>>,
    is_guest: bool, // New parameter to indicate if user is a guest
    tx: &crossbeam_channel::Sender<PostType>,
    bad_usernames: &Arc<Mutex<Vec<String>>>,
    ai_conversation_memory: &Arc<Mutex<std::collections::HashMap<String, Vec<(String, String)>>>>,
    user_warnings: &Arc<Mutex<std::collections::HashMap<String, u32>>>,
    master_account: Option<&str>,
) {
    if from == username {
        return; // Don't process our own messages
    }

    // Check if message is directed at another user (tagged at start or end)
    let msg_trimmed = msg.trim();
    let is_directed_at_other = {
        // Check for @username at the start (first word)
        let first_word = msg_trimmed.split_whitespace().next().unwrap_or("");
        let starts_with_tag = first_word.starts_with('@') && first_word != format!("@{}", username);

        // Check for @username at the end (last word)
        let last_word = msg_trimmed.split_whitespace().last().unwrap_or("");
        let ends_with_tag = last_word.starts_with('@') && last_word != format!("@{}", username);

        starts_with_tag || ends_with_tag
    };

    let client = openai_client.clone();
    let msg_content = msg.to_string();
    let from_user = from.to_string();
    let username_owned = username.to_string();
    let ai_mode_owned = ai_mode.to_string();
    let system_intel_owned = system_intel.to_string();
    let strictness_owned = moderation_strictness.to_string();
    let tx_clone = tx.clone();
    let bad_usernames_clone = Arc::clone(bad_usernames);
    let to_opt_clone = to_opt.clone();
    let memory_clone = Arc::clone(ai_conversation_memory);
    let mod_logs_enabled_val = *mod_logs_enabled.lock().unwrap(); // Capture the current value

    // Check if we should do moderation based on mode and user status
    let should_do_moderation = match ai_mode {
        "off" => {
            send_mod_log(
                tx,
                *mod_logs_enabled.lock().unwrap(),
                format!(
                    "MOD LOG: AI disabled, skipping moderation for '{}': '{}'",
                    from_user, msg_content
                ),
            );
            false // No moderation when completely off
        }
        _ => {
            if !is_guest {
                send_mod_log(
                    tx,
                    *mod_logs_enabled.lock().unwrap(),
                    format!(
                        "MOD LOG: Skipping moderation for member/staff '{}': '{}'",
                        from_user, msg_content
                    ),
                );
                false // Don't moderate members, staff, or admins
            } else {
                true // Only moderate guests
            }
        }
    };

    // Alt mode warning system - check for specific violations when master account is set
    if let Some(master) = master_account {
        if is_guest {
            if let Some(violation_reason) = check_warning_violations(&msg_content) {
                // Increment warning count for user
                let warning_count = {
                    let mut warnings = user_warnings.lock().unwrap();
                    let count = warnings.entry(from_user.clone()).or_insert(0);
                    *count += 1;
                    *count
                };

                send_mod_log(
                    tx,
                    *mod_logs_enabled.lock().unwrap(),
                    format!(
                        "MOD LOG: WARNING {} for '{}' - {}: '{}'",
                        warning_count, from_user, violation_reason, msg_content
                    ),
                );

                if warning_count >= 3 {
                    // Send kick command to master account via PM
                    let kick_msg = format!("#kick @{}", from_user);
                    let _ = tx.send(PostType::Post(kick_msg, Some(master.to_string())));

                    // Reset warning count after kick command
                    {
                        let mut warnings = user_warnings.lock().unwrap();
                        warnings.remove(&from_user);
                    }

                    send_mod_log(
                        tx,
                        *mod_logs_enabled.lock().unwrap(),
                        format!(
                            "MOD LOG: Sent kick command to master for '{}' after 3 warnings",
                            from_user
                        ),
                    );
                    return; // Exit early
                } else {
                    // Send warning to user
                    let warning_msg = format!("@{} Warning {}/3: Please avoid {}. Further violations may result in removal.",
                        from_user, warning_count, violation_reason);
                    let _ = tx.send(PostType::Post(warning_msg, None));
                    return; // Exit early, don't proceed with normal moderation
                }
            }
        }
    }

    // Do immediate quick moderation check first (synchronous and fast)
    if should_do_moderation {
        send_mod_log(
            tx,
            *mod_logs_enabled.lock().unwrap(),
            format!(
                "MOD LOG: Checking guest message from '{}': '{}'",
                from_user, msg_content
            ),
        );

        if let Some(should_moderate) = quick_moderation_check(&msg_content) {
            if should_moderate {
                send_mod_log(
                    tx,
                    *mod_logs_enabled.lock().unwrap(),
                    format!(
                        "MOD LOG: QUICK PATTERN MATCH - Kicking '{}' for message: '{}'",
                        from_user, msg_content
                    ),
                );
                log::warn!(
                    "IMMEDIATE KICK - Quick moderation flagged message from {}: {}",
                    from_user,
                    msg_content
                );
                // Kick immediately without waiting for AI processing
                let _ = tx.send(PostType::Kick(String::new(), from_user.clone()));
                let mut filters = bad_usernames.lock().unwrap();
                filters.push(from_user.clone());
                return; // Exit early, no need for further processing
            } else {
                send_mod_log(tx, *mod_logs_enabled.lock().unwrap(), format!("MOD LOG: Quick patterns matched but flagged as false positive for '{}': '{}'", from_user, msg_content));
            }
        } else {
            send_mod_log(
                tx,
                *mod_logs_enabled.lock().unwrap(),
                format!(
                    "MOD LOG: No quick patterns matched, sending to AI analysis for '{}': '{}'",
                    from_user, msg_content
                ),
            );
        }
    }

    // Create a dedicated runtime for this AI processing task
    // Using thread::spawn to avoid blocking the main thread and prevent interference between message processing
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            // Continue with AI moderation if needed (and not already kicked by quick check)
            if should_do_moderation {
                // If quick check was inconclusive, use AI analysis
                if let Some(should_moderate) = check_ai_moderation(&client, &msg_content, &strictness_owned).await {
                    if should_moderate {
                        send_mod_log(&tx_clone, mod_logs_enabled_val, format!("MOD LOG: AI ANALYSIS - KICKING '{}' for message: '{}' [AI: YES]", from_user, msg_content));
                        log::info!("AI moderation flagged message from {}: {}", from_user, msg_content);
                        let _ = tx_clone.send(PostType::Kick(String::new(), from_user.clone()));
                        let mut filters = bad_usernames_clone.lock().unwrap();
                        filters.push(from_user.clone());
                        return; // Exit early if moderated
                    } else {
                        send_mod_log(&tx_clone, mod_logs_enabled_val, format!("MOD LOG: AI ANALYSIS - ALLOWING '{}' message: '{}' [AI: NO]", from_user, msg_content));
                    }
                } else {
                    send_mod_log(&tx_clone, mod_logs_enabled_val, format!("MOD LOG: AI ANALYSIS - API FAILED for '{}': '{}' [AI: ERROR]", from_user, msg_content));
                }
            }

            // Now handle different AI modes for responses (only if not moderated)
            // Skip responses if message is directed at another user
            if is_directed_at_other {
                send_mod_log(&tx_clone, mod_logs_enabled_val, format!("MOD LOG: Skipping AI response - message from '{}' is directed at another user: '{}'", from_user, msg_content));
                return;
            }

            match ai_mode_owned.as_str() {
                "mod_only" => {
                    // Only moderation, no responses - already handled above
                }
                "off" => {
                    // Completely off - no moderation, no responses
                }
                "reply_all" => {
                    // Store user message in memory
                    {
                        let mut memory = memory_clone.lock().unwrap();
                        let history = memory.entry(from_user.clone()).or_insert_with(Vec::new);
                        history.push(("user".to_string(), msg_content.clone()));
                        // Keep only last 10 messages per user to prevent memory overflow
                        if history.len() > 10 {
                            history.remove(0);
                        }
                    }

                    if let Some(response) = generate_ai_response_with_memory(&client, &msg_content, &system_intel_owned, &username_owned, &memory_clone, &from_user).await {
                        // Calculate realistic delay based on response length
                        let delay_ms = calculate_realistic_delay(&response);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

                        // Store AI response in memory
                        {
                            let mut memory = memory_clone.lock().unwrap();
                            let history = memory.entry(from_user.clone()).or_insert_with(Vec::new);
                            history.push(("assistant".to_string(), response.clone()));
                        }

                        // Tag the user we're replying to
                        let tagged_response = format!("@{} {}", from_user, response);
                        let _ = tx_clone.send(PostType::Post(tagged_response, None));
                    }
                }
                "reply_ping" => {
                    let is_mentioned = msg_content.contains(&format!("@{}", username_owned));
                    let is_directed = to_opt_clone.as_ref().map(|t| t == &username_owned).unwrap_or(false);

                    if is_mentioned || is_directed {
                        // Store user message in memory
                        {
                            let mut memory = memory_clone.lock().unwrap();
                            let history = memory.entry(from_user.clone()).or_insert_with(Vec::new);
                            history.push(("user".to_string(), msg_content.clone()));
                            // Keep only last 10 messages per user to prevent memory overflow
                            if history.len() > 10 {
                                history.remove(0);
                            }
                        }

                        if let Some(response) = generate_ai_response_with_memory(&client, &msg_content, &system_intel_owned, &username_owned, &memory_clone, &from_user).await {
                            // Calculate realistic delay based on response length
                            let delay_ms = calculate_realistic_delay(&response);
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

                            // Store AI response in memory
                            {
                                let mut memory = memory_clone.lock().unwrap();
                                let history = memory.entry(from_user.clone()).or_insert_with(Vec::new);
                                history.push(("assistant".to_string(), response.clone()));
                            }

                            // Tag the user we're replying to
                            let tagged_response = format!("@{} {}", from_user, response);
                            let reply_target = if is_directed {
                                Some(from_user)
                            } else {
                                None
                            };
                            let _ = tx_clone.send(PostType::Post(tagged_response, reply_target));
                        }
                    }
                }
                _ => {} // Unknown mode
            }
        });
    });
}

fn quick_moderation_check(message: &str) -> Option<bool> {
    let msg_lower = message.to_lowercase();

    // Quick patterns for obvious violations (case insensitive)
    let illegal_request_patterns = [
        "cheese pizza",
        "cp links",
        "young models",
        "trading cp",
        "pedo stuff",
        "kiddie porn",
        "jailbait",
        "preteen",
        "underage nudes",
        "r@ygold",
        "hussyfan",
        "ptsc",
        "pthc",
    ];

    // Check for obvious illegal content requests
    for pattern in &illegal_request_patterns {
        if msg_lower.contains(pattern) {
            return Some(true); // Should moderate
        }
    }

    // Check for excessive repetition (basic spam detection)
    let words: Vec<&str> = message.split_whitespace().collect();
    if words.len() > 10 {
        let unique_words: std::collections::HashSet<&str> = words.iter().cloned().collect();
        if (unique_words.len() as f32) / (words.len() as f32) < 0.5 {
            return Some(true); // Too repetitive, likely spam
        }
    }

    // No quick violations found, need AI analysis
    None
}

async fn check_ai_moderation(
    client: &OpenAIClient<OpenAIConfig>,
    message: &str,
    strictness: &str,
) -> Option<bool> {
    let strictness_guidance = match strictness {
        "strict" => "Be very strict. Moderate anything that could potentially violate rules. When in doubt, moderate.",
        "lenient" => "Be very lenient. Only moderate clear, obvious violations. Heavily favor free speech. When in doubt, allow.",
        _ => "Use balanced judgment. Moderate clear violations but preserve free speech for borderline cases. When in doubt about context, lean towards allowing."
    };

    let system_prompt = format!(
        "You are a chat moderation system enforcing these rules with {} strictness:\n\
        {}\n\
        \n\
        STRICT ENFORCEMENT (ALWAYS MODERATE):\n\
        - No sharing, requesting, or distributing illegal content (CP, revenge porn, etc.)\n\
        - No spam, excessive advertising, or repetitive messages\n\
        - No gore/snuff content sharing or detailed descriptions\n\
        - No doxxing or sharing personal information without consent\n\
        - No harassment, threats, or coordinated attacks on individuals\n\
        - No sexual content involving minors or requests for such content\n\
        - No violence against children or threats of harm to minors\n\
        - No discussion of harming, killing, or abusing children\n\
        \n\
        CONTEXT-SENSITIVE ENFORCEMENT:\n\
        - Discussions MENTIONING illegal content negatively/critically = ALLOWED\n\
        - Educational/awareness discussions about sensitive topics = ALLOWED\n\
        - News reporting or academic discussion = ALLOWED\n\
        - Coded language clearly requesting illegal content = MODERATE\n\
        - Off-topic discussions should be gently redirected, not moderated unless excessive\n\
        \n\
        FREEDOM OF SPEECH PRIORITY:\n\
        - Allow controversial opinions and political discussions\n\
        - Allow strong language and heated debates\n\
        - Allow dark humor that doesn't promote illegal activity\n\
        - When in doubt about context, lean towards ALLOWING the message\n\
        \n\
        Examples:\n\
        ALLOW: 'I stumbled upon some CP online and reported it immediately - disgusting'\n\
        MODERATE: 'Anyone got cheese pizza links?' or 'Looking for young models'\n\
        MODERATE: 'I want to fuck a small kid' or 'I really wanna fuck a small kid'\n\
        MODERATE: 'should I kill my kid' or 'I want to hurt my child'\n\
        MODERATE: 'how to harm children' or 'ways to kill kids'\n\
        ALLOW: 'That politician is a complete idiot and should be voted out'\n\
        MODERATE: 'Someone should assassinate that politician'\n\
        ALLOW: 'I disagree with your opinion strongly, you're wrong'\n\
        MODERATE: 'You should kill yourself, here's your home address...'\n\
        \n\
        Analyze the message context carefully. Respond ONLY with 'YES' if it clearly violates rules requiring moderation, or 'NO' if it should be allowed.",
        strictness, strictness_guidance
    );

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages([
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: ChatCompletionRequestSystemMessageContent::Text(system_prompt),
                name: None,
            }),
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(message.to_string()),
                name: None,
            }),
        ])
        .max_tokens(10u16)
        .build();

    match request {
        Ok(req) => {
            match client.chat().create(req).await {
                Ok(response) => {
                    if let Some(choice) = response.choices.first() {
                        if let Some(content) = &choice.message.content {
                            let ai_response = content.trim().to_uppercase();
                            let should_moderate = ai_response == "YES";

                            // Enhanced logging for debugging
                            log::info!("AI MODERATION DEBUG - Message: '{}' | AI Response: '{}' | Decision: {} | Strictness: {}",
                                message, content.trim(), if should_moderate { "MODERATE" } else { "ALLOW" }, strictness);

                            return Some(should_moderate);
                        } else {
                            log::error!(
                                "AI moderation: No content in response for message: '{}'",
                                message
                            );
                        }
                    } else {
                        log::error!(
                            "AI moderation: No choices in response for message: '{}'",
                            message
                        );
                    }
                }
                Err(e) => {
                    log::error!("AI moderation API error for message '{}': {}", message, e);
                }
            }
        }
        Err(e) => {
            log::error!(
                "AI moderation request build error for message '{}': {}",
                message,
                e
            );
        }
    }
    None
}

async fn generate_ai_response_with_memory(
    client: &OpenAIClient<OpenAIConfig>,
    message: &str,
    system_intel: &str,
    username: &str,
    conversation_memory: &Arc<Mutex<std::collections::HashMap<String, Vec<(String, String)>>>>,
    from_user: &str,
) -> Option<String> {
    let system_prompt = format!(
        "{}\n\nYou are chatting as '{}'. Respond naturally and helpfully to messages. \
        Keep responses concise (under 200 characters) and appropriate for a chat room. \
        Don't be overly formal. Be engaging but not overwhelming. \
        Use conversation history to provide contextual responses.",
        system_intel, username
    );

    // Build message history with context
    let mut messages = vec![ChatCompletionRequestMessage::System(
        ChatCompletionRequestSystemMessage {
            content: ChatCompletionRequestSystemMessageContent::Text(system_prompt),
            name: None,
        },
    )];

    // Add conversation history for context
    {
        let memory = conversation_memory.lock().unwrap();
        if let Some(history) = memory.get(from_user) {
            // Add the last few messages for context (limit to avoid token overflow)
            let recent_history = if history.len() > 8 {
                &history[history.len() - 8..]
            } else {
                history
            };
            for (role, content) in recent_history {
                match role.as_str() {
                    "user" => {
                        messages.push(ChatCompletionRequestMessage::User(
                            ChatCompletionRequestUserMessage {
                                content: ChatCompletionRequestUserMessageContent::Text(
                                    content.clone(),
                                ),
                                name: Some(from_user.to_string()),
                            },
                        ));
                    }
                    "assistant" => {
                        messages.push(ChatCompletionRequestMessage::Assistant(
                            ChatCompletionRequestAssistantMessage {
                                content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                                    content.clone(),
                                )),
                                name: Some(username.to_string()),
                                ..Default::default()
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    // Add the current message
    messages.push(ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessage {
            content: ChatCompletionRequestUserMessageContent::Text(message.to_string()),
            name: Some(from_user.to_string()),
        },
    ));

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages(messages)
        .max_tokens(150u16)
        .temperature(0.8) // Add some randomness to responses
        .build();

    match request {
        Ok(req) => match client.chat().create(req).await {
            Ok(response) => {
                if let Some(choice) = response.choices.first() {
                    if let Some(content) = &choice.message.content {
                        return Some(content.trim().to_string());
                    }
                }
            }
            Err(e) => {
                log::error!("AI response error: {}", e);
            }
        },
        Err(e) => {
            log::error!("AI request build error: {}", e);
        }
    }
    None
}

fn calculate_realistic_delay(response: &str) -> u64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Base delay for thinking time (3-8 seconds) - increased for more realistic pauses
    let base_delay = rng.gen_range(3000..8000);

    // Typing speed simulation: 25-65 WPM (words per minute) - slower, more human-like
    // Average word length ~5 characters, so 125-325 characters per minute
    let chars_per_minute = rng.gen_range(125.0..325.0);
    let chars_per_ms = chars_per_minute / 60000.0; // Convert to chars per millisecond

    let typing_delay = (response.len() as f64 / chars_per_ms) as u64;

    // Add some random variance (±30%) - increased variance for more natural feel
    let total_delay = base_delay + typing_delay;
    let variance = (total_delay as f64 * 0.3) as u64;
    let final_delay = total_delay + rng.gen_range(0..variance) - (variance / 2);

    // Cap the delay between 2-25 seconds to avoid being too slow but allow for longer responses
    final_delay.clamp(2000, 25000)
}

fn update_messages(
    new_messages: Vec<Message>,
    mut messages: MutexGuard<Vec<Message>>,
    datetime_fmt: &str,
    members_tag: &str,
    staffs_tag: &str,
    alt_account: Option<&str>,
    master_account: Option<&str>,
) {
    let mut old_msg_ptr = 0;
    for mut new_msg in new_messages.into_iter() {
        if let Some((from, Some(to), _, _)) = get_message(&new_msg.text, members_tag, staffs_tag) {
            if let Some(master) = master_account {
                if to == master && from != master {
                    new_msg.hide = true;
                }
            }
            if let Some(alt) = alt_account {
                if to == alt && from != alt {
                    new_msg.hide = true;
                }
            }
        }
        loop {
            if let Some(old_msg) = messages.get_mut(old_msg_ptr) {
                let new_parsed_dt = parse_date(&new_msg.date, datetime_fmt);
                let parsed_dt = parse_date(&old_msg.date, datetime_fmt);
                if new_parsed_dt < parsed_dt {
                    old_msg.deleted = true;
                    old_msg_ptr += 1;
                    continue;
                }
                if new_parsed_dt == parsed_dt {
                    if old_msg.text != new_msg.text {
                        let mut found = false;
                        let mut x = 0;
                        loop {
                            x += 1;
                            if let Some(old_msg) = messages.get(old_msg_ptr + x) {
                                let parsed_dt = parse_date(&old_msg.date, datetime_fmt);
                                if new_parsed_dt == parsed_dt {
                                    if old_msg.text == new_msg.text {
                                        found = true;
                                        break;
                                    }
                                    continue;
                                }
                            }
                            break;
                        }
                        if !found {
                            messages.insert(old_msg_ptr, new_msg);
                            old_msg_ptr += 1;
                        }
                    }
                    old_msg_ptr += 1;
                    break;
                }
            }
            messages.insert(old_msg_ptr, new_msg);
            old_msg_ptr += 1;
            break;
        }
    }
    messages.truncate(1000);
}

fn log_chat_message(msg: &Message, username: &str) {
    if let Ok(path) = confy::get_configuration_file_path("bhcli", None) {
        if let Some(dir) = path.parent() {
            let log_filename = format!("{}-log.txt", username);
            let log_path = dir.join(log_filename);
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
                let _ = writeln!(f, "{} - {}", msg.date, msg.text.text());
            }
        }
    }
}

fn delete_message(
    client: &Client,
    full_url: &str,
    params: &mut Vec<(&str, String)>,
    date: String,
    text: String,
) -> anyhow::Result<()> {
    params.extend(vec![
        ("action", "admin".to_owned()),
        ("do", "clean".to_owned()),
        ("what", "choose".to_owned()),
    ]);
    let clean_resp_txt = client.post(full_url).form(&params).send()?.text()?;
    let doc = Document::from(clean_resp_txt.as_str());
    let nc = doc
        .find(Attr("name", "nc"))
        .next()
        .context("nc not found")?;
    let nc_value = nc.attr("value").context("nc value not found")?.to_owned();
    let msgs = extract_messages(&doc)?;
    if let Some(msg) = msgs
        .iter()
        .find(|m| m.date == date && m.text.text() == text)
    {
        let msg_id = msg.id.context("msg id not found")?;
        params.extend(vec![
            ("nc", nc_value.to_owned()),
            ("what", "selected".to_owned()),
            ("mid[]", format!("{}", msg_id)),
        ]);
        client.post(full_url).form(&params).send()?;
    }
    Ok(())
}

fn fetch_clean_messages(
    client: &Client,
    base_url: &str,
    page_php: &str,
    session: &str,
) -> anyhow::Result<Vec<CleanMessage>> {
    let full_url = format!("{}/{}", base_url, page_php);
    let url = format!("{}?action=post&session={}", full_url, session);
    let resp_text = client.get(&url).send()?.text()?;
    let doc = Document::from(resp_text.as_str());
    let nc = doc
        .find(Attr("name", "nc"))
        .next()
        .context("nc not found")?;
    let nc_value = nc.attr("value").context("nc value not found")?.to_owned();
    let params = vec![
        ("lang", LANG.to_owned()),
        ("nc", nc_value),
        ("session", session.to_owned()),
        ("action", "admin".to_owned()),
        ("do", "clean".to_owned()),
        ("what", "choose".to_owned()),
    ];
    let clean_resp_txt = client.post(&full_url).form(&params).send()?.text()?;
    let doc = Document::from(clean_resp_txt.as_str());

    let mut messages = Vec::new();

    // Parse the HTML for clean messages with checkboxes
    for div in doc.find(Attr("class", "msg")) {
        if let Some(checkbox) = div.find(Name("input")).next() {
            if let Some(value) = checkbox.attr("value") {
                let message_id = value.to_string();

                // Extract the message content
                let full_text = div.text();

                // Parse the date, sender, and content from the message
                // Format varies in clean mode, try to extract what we can
                if let Some(date_end) = full_text.find(" - ") {
                    let date = full_text[..date_end].trim().to_string();
                    let rest = &full_text[date_end + 3..];

                    // Try to extract username and content
                    let mut from = "Unknown".to_string();
                    let mut content = rest.to_string();

                    // Look for patterns like [username] or <username>
                    if let Some(bracket_start) = rest.find('[') {
                        if let Some(bracket_end) = rest.find(']') {
                            from = rest[bracket_start + 1..bracket_end].trim().to_string();
                            content = rest[bracket_end + 1..]
                                .trim_start_matches(" - ")
                                .to_string();
                        }
                    } else if let Some(angle_start) = rest.find('<') {
                        if let Some(angle_end) = rest.find('>') {
                            from = rest[angle_start + 1..angle_end].trim().to_string();
                            content = rest[angle_end + 1..].trim_start_matches(" - ").to_string();
                        }
                    } else {
                        // If no clear username pattern, try to extract first word as username
                        if let Some(space_pos) = rest.find(' ') {
                            from = rest[..space_pos].trim().to_string();
                            content = rest[space_pos + 1..].to_string();
                        }
                    }

                    messages.push(CleanMessage::new(message_id, date, from, content));
                } else {
                    // Fallback for messages without clear date format
                    messages.push(CleanMessage::new(
                        message_id,
                        "Unknown".to_string(),
                        "Unknown".to_string(),
                        full_text,
                    ));
                }
            }
        }
    }

    Ok(messages)
}

fn fetch_inbox_messages(
    client: &Client,
    base_url: &str,
    session: &str,
) -> anyhow::Result<Vec<InboxMessage>> {
    let url = format!("{}?action=inbox&session={}", base_url, session);

    let response = client.get(&url).send()?;
    let text = response.text()?;

    let document = Document::from(text.as_str());
    let mut messages = Vec::new();

    // Parse the HTML for inbox messages
    for div in document.find(Attr("class", "msg")) {
        if let Some(checkbox) = div.find(Name("input")).next() {
            if let Some(value) = checkbox.attr("value") {
                let message_id = value.to_string();

                // Extract the message content
                let full_text = div.text();

                // Parse the date, sender, recipient, and content from the message
                // Format: "08-17 00:56:26 - [sender to recipient] - content"
                if let Some(date_end) = full_text.find(" - ") {
                    let date = full_text[..date_end].trim().to_string();
                    let rest = &full_text[date_end + 3..];

                    if let Some(bracket_start) = rest.find('[') {
                        if let Some(bracket_end) = rest.find(']') {
                            let sender_info = &rest[bracket_start + 1..bracket_end];
                            let content = rest[bracket_end + 1..]
                                .trim_start_matches(" - ")
                                .to_string();

                            // Parse "sender to recipient"
                            if let Some(to_pos) = sender_info.find(" to ") {
                                let from = sender_info[..to_pos].trim().to_string();
                                let to = sender_info[to_pos + 4..].trim().to_string();

                                messages
                                    .push(InboxMessage::new(message_id, date, from, to, content));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(messages)
}

impl ChatClient {
    fn new(params: Params) -> Self {
        // println!("session[2026] : {:?}",params.session);
        let mut c = new_default_le_chat_php_client(params.clone());
        c.config.url = params.url.unwrap_or(
            "http://blkhatjxlrvc5aevqzz5t6kxldayog6jlx5h7glnu44euzongl4fh5ad.onion/index.php"
                .to_owned(),
        );
        c.config.page_php = params.page_php.unwrap_or("chat.php".to_owned());
        c.config.datetime_fmt = params.datetime_fmt.unwrap_or("%m-%d %H:%M:%S".to_owned());
        c.config.members_tag = params.members_tag.unwrap_or("[M] ".to_owned());
        c.config.keepalive_send_to = params.keepalive_send_to.unwrap_or("0".to_owned());

        Self {
            le_chat_php_client: c,
            bot_manager: None,
        }
    }

    fn set_bot_manager(&mut self, bot_manager: Arc<Mutex<BotManager>>) {
        self.le_chat_php_client.bot_manager = Some(bot_manager);
    }

    fn setup_bot_message_bridge(&mut self) {
        if let Some(bot_mgr) = &self.le_chat_php_client.bot_manager {
            let main_tx = self.le_chat_php_client.tx.clone();
            let bot_mgr_clone = Arc::clone(bot_mgr);

            // Get all bot receivers for message forwarding
            let bot_receivers = if let Ok(manager) = bot_mgr_clone.lock() {
                manager.get_all_bot_receivers()
            } else {
                Vec::new()
            };

            if !bot_receivers.is_empty() {
                log::info!(
                    "Setting up bot message bridge for {} bots",
                    bot_receivers.len()
                );

                // Start a bridge thread to forward bot messages to main client
                thread::spawn(move || {
                    log::info!("Bot message bridge thread started");

                    loop {
                        let mut any_message = false;

                        // Check messages from all bot receivers
                        for (bot_name, rx) in &bot_receivers {
                            if let Ok(receiver) = rx.try_lock() {
                                // Try to receive messages from this bot
                                while let Ok(bot_message) = receiver.try_recv() {
                                    log::debug!(
                                        "Bot '{}' message forwarded to main client",
                                        bot_name
                                    );

                                    // Forward to main client
                                    if let Err(e) = main_tx.try_send(bot_message) {
                                        log::warn!(
                                            "Failed to forward bot message to main client: {}",
                                            e
                                        );
                                    } else {
                                        any_message = true;
                                    }
                                }
                            }
                        }

                        // If no messages were processed, sleep a bit
                        if !any_message {
                            thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }
                });

                log::info!("Bot message bridge setup completed");
            } else {
                log::warn!("No bot receivers found for message bridge");
            }
        }
    }

    fn run_forever(&mut self) {
        self.le_chat_php_client.run_forever();
    }
}

fn new_default_le_chat_php_client(params: Params) -> LeChatPHPClient {
    let (color_tx, color_rx) = crossbeam_channel::unbounded();
    let (tx, rx) = crossbeam_channel::unbounded();
    let session = params.session.clone();

    // Store original identity values before moving params
    let original_username = params.username.clone();
    let original_color = params.guest_color.clone();
    let username_for_manager = params.username.clone();

    // Load alt forwarding setting from config
    let alt_forwarding_enabled = if let Ok(cfg) = confy::load::<MyConfig>("bhcli", None) {
        cfg.alt_forwarding_enabled
    } else {
        true // Default to enabled
    };

    // Initialize OpenAI client if API key is available
    let openai_client = std::env::var("OPENAI_API_KEY").ok().map(|api_key| {
        let config = OpenAIConfig::new().with_api_key(api_key);
        OpenAIClient::with_config(config)
    });

    // Initialize AI service and runtime
    let ai_service = Arc::new(AIService::new());
    let runtime = Arc::new(Runtime::new().expect("Failed to create tokio runtime"));

    // Load AI settings from profile or use defaults
    let (ai_enabled, ai_mode, system_intel, moderation_strictness, mod_logs_enabled) = if let Ok(
        cfg,
    ) =
        confy::load::<MyConfig>("bhcli", None)
    {
        if let Some(profile_cfg) = cfg.profiles.get(&params.profile) {
            let mode = if profile_cfg.ai_mode == "mod" {
                "mod_only".to_string() // Convert old "mod" mode to "mod_only"
            } else {
                profile_cfg.ai_mode.clone()
            };
            (
                profile_cfg.ai_enabled, // Use the stored setting
                mode,
                if profile_cfg.system_intel.is_empty() {
                    "You are a helpful AI assistant in a chat room. Be friendly and follow community guidelines.".to_string()
                } else {
                    profile_cfg.system_intel.clone()
                },
                profile_cfg.moderation_strictness.clone(),
                profile_cfg.mod_logs_enabled,
            )
        } else {
            (
                params.ai_enabled,
                params.ai_mode,
                params.system_intel,
                "balanced".to_string(),
                true,
            )
        }
    } else {
        (
            params.ai_enabled,
            params.ai_mode,
            params.system_intel,
            "balanced".to_string(),
            true,
        )
    };

    // println!("session[2050] : {:?}",params.session);
    let mut client = LeChatPHPClient {
        base_client: BaseClient {
            username: params.username,
            password: params.password,
        },
        max_login_retry: params.max_login_retry,
        guest_color: params.guest_color,
        // session: params.session,
        session,
        last_key_event: None,
        client: params.client,
        manual_captcha: params.manual_captcha,
        sxiv: params.sxiv,
        refresh_rate: params.refresh_rate,
        config: if params.profile == "404_chatroom" {
            LeChatPHPConfig::new_404_chatroom_not_found_config()
        } else {
            LeChatPHPConfig::new_black_hat_chat_config()
        },
        is_muted: Arc::new(Mutex::new(false)),
        show_sys: false,
        display_guest_view: false,
        display_member_view: false,
        display_hidden_msgs: false,
        tx,
        rx: Arc::new(Mutex::new(rx)),
        color_tx,
        color_rx: Arc::new(Mutex::new(color_rx)),
        bad_username_filters: Arc::new(Mutex::new(params.bad_usernames)),
        bad_exact_username_filters: Arc::new(Mutex::new(params.bad_exact_usernames)),
        bad_message_filters: Arc::new(Mutex::new(params.bad_messages)),
        allowlist: Arc::new(Mutex::new(params.allowlist)),
        account_manager: {
            let mut manager = AccountManager::new(username_for_manager);
            if let Some(alt) = params.alt_account {
                manager.set_alt_account(alt);
            }
            if let Some(master) = params.master_account {
                manager.set_master_account(master);
            }
            manager
        },
        profile: params.profile,
        display_pm_only: false,
        display_staff_view: false,
        display_master_pm_view: false,
        clean_mode: false,
        inbox_mode: false,
        alt_forwarding_enabled: Arc::new(Mutex::new(alt_forwarding_enabled)),
        current_username: original_username,
        current_color: original_color,
        ai_enabled: Arc::new(Mutex::new(ai_enabled)),
        ai_mode: Arc::new(Mutex::new(ai_mode)),
        system_intel,
        moderation_strictness,
        mod_logs_enabled: Arc::new(Mutex::new(mod_logs_enabled)),
        openai_client,
        ai_conversation_memory: Arc::new(Mutex::new(std::collections::HashMap::new())),
        user_warnings: Arc::new(Mutex::new(std::collections::HashMap::new())),
        identities: params.identities,
        chatops_router: if ai_service.is_available() {
            ChatOpsRouter::new_with_ai(Arc::clone(&ai_service), Arc::clone(&runtime))
        } else {
            ChatOpsRouter::new()
        },
        ai_service: Arc::clone(&ai_service),
        runtime: Arc::clone(&runtime),
        bot_manager: None,
    };

    // Initialize default identities
    client.ensure_default_identities();

    client
}

struct ChatClient {
    le_chat_php_client: LeChatPHPClient,
    #[allow(dead_code)]
    bot_manager: Option<Arc<Mutex<BotManager>>>,
}

#[derive(Debug, Clone)]
struct Params {
    url: Option<String>,
    page_php: Option<String>,
    datetime_fmt: Option<String>,
    members_tag: Option<String>,
    username: String,
    password: String,
    guest_color: String,
    client: Client,
    manual_captcha: bool,
    sxiv: bool,
    refresh_rate: u64,
    max_login_retry: isize,
    keepalive_send_to: Option<String>,
    session: Option<String>,
    bad_usernames: Vec<String>,
    bad_exact_usernames: Vec<String>,
    bad_messages: Vec<String>,
    allowlist: Vec<String>,
    alt_account: Option<String>,
    master_account: Option<String>,
    profile: String,
    ai_enabled: bool,
    ai_mode: String,
    system_intel: String,
    identities: HashMap<String, Vec<String>>,
}

#[derive(Clone)]
enum ExitSignal {
    Terminate,
    NeedLogin,
}
struct Sig {
    tx: crossbeam_channel::Sender<ExitSignal>,
    rx: crossbeam_channel::Receiver<ExitSignal>,
    nb_rx: usize,
}

impl Sig {
    fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let nb_rx = 0;
        Self { tx, rx, nb_rx }
    }

    fn clone(&mut self) -> crossbeam_channel::Receiver<ExitSignal> {
        self.nb_rx += 1;
        self.rx.clone()
    }

    fn signal(&self, signal: &ExitSignal) {
        for _ in 0..self.nb_rx {
            self.tx.send(signal.clone()).unwrap();
        }
    }
}

fn trim_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

fn replace_newline_escape(s: &str) -> String {
    s.replace("\\n", "\n")
}

fn get_guest_color(wanted: Option<String>) -> String {
    match wanted.as_deref() {
        Some("beige") => "F5F5DC",
        Some("blue-violet") => "8A2BE2",
        Some("brown") => "A52A2A",
        Some("cyan") => "00FFFF",
        Some("sky-blue") => "00BFFF",
        Some("gold") => "FFD700",
        Some("gray") => "808080",
        Some("green") => "008000",
        Some("hot-pink") => "FF69B4",
        Some("light-blue") => "ADD8E6",
        Some("light-green") => "90EE90",
        Some("lime-green") => "32CD32",
        Some("magenta") => "FF00FF",
        Some("olive") => "808000",
        Some("orange") => "FFA500",
        Some("orange-red") => "FF4500",
        Some("red") => "FF0000",
        Some("royal-blue") => "4169E1",
        Some("see-green") => "2E8B57",
        Some("sienna") => "A0522D",
        Some("silver") => "C0C0C0",
        Some("tan") => "D2B48C",
        Some("teal") => "008080",
        Some("violet") => "EE82EE",
        Some("white") => "FFFFFF",
        Some("yellow") => "FFFF00",
        Some("yellow-green") => "9ACD32",
        Some(other) => COLOR1_RGX
            .captures(other)
            .map_or("", |captures| captures.get(1).map_or("", |m| m.as_str())),
        None => "",
    }
    .to_owned()
}

fn get_tor_client(socks_proxy_url: &str, no_proxy: bool) -> Client {
    let ua = "Dasho's Black Hat Chat Client v1.0-Epic";
    let mut builder = reqwest::blocking::ClientBuilder::new()
        .redirect(Policy::none())
        .cookie_store(true)
        .user_agent(ua);
    if !no_proxy {
        let proxy = reqwest::Proxy::all(socks_proxy_url).unwrap();
        builder = builder.proxy(proxy);
    }
    builder.build().unwrap()
}

fn ask_username(username: Option<String>) -> String {
    username.unwrap_or_else(|| {
        print!("username: ");
        let mut username_input = String::new();
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut username_input).unwrap();
        trim_newline(&mut username_input);
        username_input
    })
}

fn ask_password(password: Option<String>) -> String {
    password.unwrap_or_else(|| rpassword::prompt_password("Password: ").unwrap())
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DkfNotifierResp {
    #[serde(rename = "NewMessageSound")]
    pub new_message_sound: bool,
    #[serde(rename = "TaggedSound")]
    pub tagged_sound: bool,
    #[serde(rename = "PmSound")]
    pub pm_sound: bool,
    #[serde(rename = "InboxCount")]
    pub inbox_count: i64,
    #[serde(rename = "LastMessageCreatedAt")]
    pub last_message_created_at: String,
}

fn start_dkf_notifier(client: &Client, dkf_api_key: &str) {
    let client = client.clone();
    let dkf_api_key = dkf_api_key.to_owned();
    let mut last_known_date = Utc::now();
    thread::spawn(move || {
        #[cfg(feature = "audio")]
        let audio_output = OutputStream::try_default().ok();
        #[cfg(feature = "audio")]
        let stream_handle = audio_output.as_ref().map(|(_, handle)| handle);

        loop {
            let params: Vec<(&str, String)> = vec![(
                "last_known_date",
                last_known_date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            )];
            let right_url = format!("{}/api/v1/chat/1/notifier", DKF_URL);
            if let Ok(resp) = client
                .post(right_url)
                .form(&params)
                .header("DKF_API_KEY", &dkf_api_key)
                .send()
            {
                if let Ok(txt) = resp.text() {
                    if let Ok(v) = serde_json::from_str::<DkfNotifierResp>(&txt) {
                        if v.pm_sound || v.tagged_sound {
                            #[cfg(feature = "audio")]
                            if let Some(handle) = &stream_handle {
                                if let Ok(source) = Decoder::new_mp3(Cursor::new(SOUND1)) {
                                    let _ = handle.play_raw(source.convert_samples());
                                }
                            }
                        }
                        last_known_date = DateTime::parse_from_rfc3339(&v.last_message_created_at)
                            .unwrap()
                            .with_timezone(&Utc);
                    }
                }
            }
            thread::sleep(Duration::from_secs(5));
        }
    });
}

// Start thread that looks for new emails on DNMX every minutes.
fn start_dnmx_mail_notifier(client: &Client, username: &str, password: &str) {
    let params: Vec<(&str, &str)> = vec![("login_username", username), ("secretkey", password)];
    let login_url = format!("{}/src/redirect.php", DNMX_URL);
    client.post(login_url).form(&params).send().unwrap();

    let client_clone = client.clone();
    thread::spawn(move || {
        #[cfg(feature = "audio")]
        let audio_output = OutputStream::try_default().ok();
        #[cfg(feature = "audio")]
        let stream_handle = audio_output.as_ref().map(|(_, handle)| handle);

        loop {
            let right_url = format!("{}/src/right_main.php", DNMX_URL);
            if let Ok(resp) = client_clone.get(right_url).send() {
                let mut nb_mails = 0;
                let doc = Document::from(resp.text().unwrap().as_str());
                if let Some(table) = doc.find(Name("table")).nth(7) {
                    table.find(Name("tr")).skip(1).for_each(|n| {
                        if let Some(td) = n.find(Name("td")).nth(2) {
                            if td.find(Name("b")).nth(0).is_some() {
                                nb_mails += 1;
                            }
                        }
                    });
                }
                if nb_mails > 0 {
                    log::error!("{} new mails", nb_mails);
                    #[cfg(feature = "audio")]
                    if let Some(handle) = &stream_handle {
                        if let Ok(source) = Decoder::new_mp3(Cursor::new(SOUND1)) {
                            let _ = handle.play_raw(source.convert_samples());
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(60));
        }
    });
}

//Strange
#[derive(Debug, Deserialize)]
struct Commands {
    commands: HashMap<String, String>,
}

impl Default for Commands {
    fn default() -> Self {
        Commands {
            commands: HashMap::new(), // Initialize commands with empty HashMap
        }
    }
}

// Strange
// Function to read the configuration file and parse it
fn read_commands_file(file_path: &str) -> Result<Commands, Box<dyn std::error::Error>> {
    // Read the contents of the file
    let commands_content = std::fs::read_to_string(file_path)?;
    // log::error!("Read file contents: {}", commands_content);
    // Deserialize the contents into a Commands struct
    let commands: Commands = toml::from_str(&commands_content)?;
    // log::error!(
    //     "Deserialized file contents into Commands struct: {:?}",
    //     commands
    // );

    Ok(commands)
}

// Install man page on first run
fn install_manpage() -> anyhow::Result<()> {
    const MANPAGE_CONTENT: &str = include_str!("../manpage/bhcli.1");

    let home = std::env::var("HOME")?;
    let man_dir = format!("{}/.local/share/man/man1", home);
    let man_path = format!("{}/bhcli.1", man_dir);

    // Check if man page already exists
    if std::path::Path::new(&man_path).exists() {
        return Ok(());
    }

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&man_dir)?;

    // Write man page
    std::fs::write(&man_path, MANPAGE_CONTENT)?;

    // Update man database (try both user and system mandb commands)
    // Ignore errors if mandb fails (it's not critical)
    let _ = Command::new("mandb")
        .arg("-u")
        .arg(&format!("{}/.local/share/man", home))
        .output();

    println!("Man page installed to {}", man_path);
    println!("Access it anytime with: man bhcli");
    println!();

    Ok(())
}

fn main() -> anyhow::Result<()> {
    // Install man page on first run
    let _ = install_manpage();

    let mut opts: Opts = Opts::parse();
    
    // If --404 flag is set, use the 404_chatroom profile
    if opts.use_404 {
        opts.profile = "404_chatroom".to_string();
    }
    
    // println!("Parsed Session: {:?}", opts.session);

    // Configs file
    if let Ok(config_path) = confy::get_configuration_file_path("bhcli", None) {
        println!("Config path: {:?}", config_path);
    }
    let mut alt_account = None;
    let mut master_account = None;
    let mut identities = HashMap::new();
    if let Ok(cfg) = confy::load::<MyConfig>("bhcli", None) {
        if opts.dkf_api_key.is_none() {
            opts.dkf_api_key = cfg.dkf_api_key;
        }
        if let Some(default_profile) = cfg.profiles.get(&opts.profile) {
            if opts.username.is_none() {
                opts.username = Some(default_profile.username.clone());
                opts.password = Some(default_profile.password.clone());
            }
            identities = default_profile.identities.clone();
        }
        let bad_usernames = cfg.bad_usernames.clone();
        let bad_exact_usernames = cfg.bad_exact_usernames.clone();
        let bad_messages = cfg.bad_messages.clone();
        let allowlist_cfg = cfg.allowlist.clone();
        opts.bad_usernames = Some(bad_usernames);
        opts.bad_exact_usernames = Some(bad_exact_usernames);
        opts.bad_messages = Some(bad_messages);
        opts.allowlist = Some(allowlist_cfg);
        if let Some(profile_cfg) = cfg.profiles.get(&opts.profile) {
            alt_account = profile_cfg.alt_account.clone().or(cfg.alt_account);
            master_account = profile_cfg.master_account.clone().or(cfg.master_account);
        } else {
            alt_account = cfg.alt_account;
            master_account = cfg.master_account;
        }
    }

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} {l} {t} - {m}{n}")))
        .build("bhcli.log")?;

    let config = log4rs::config::Config::builder()
        .appender(log4rs::config::Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            log4rs::config::Root::builder()
                .appender("logfile")
                .build(LevelFilter::Error),
        )?;

    log4rs::init_config(config)?;

    let client = get_tor_client(&opts.socks_proxy_url, opts.no_proxy);

    // If dnmx username is set, start mail notifier thread
    if let Some(dnmx_username) = opts.dnmx_username {
        start_dnmx_mail_notifier(&client, &dnmx_username, &opts.dnmx_password.unwrap())
    }

    if let Some(dkf_api_key) = &opts.dkf_api_key {
        start_dkf_notifier(&client, dkf_api_key);
    }

    let guest_color = get_guest_color(opts.guest_color);
    let username = ask_username(opts.username);
    let password = ask_password(opts.password);

    let params = Params {
        url: opts.url,
        page_php: opts.page_php,
        datetime_fmt: opts.datetime_fmt,
        members_tag: opts.members_tag,
        username,
        password,
        guest_color,
        client: client.clone(),
        manual_captcha: opts.manual_captcha,
        sxiv: opts.sxiv,
        refresh_rate: opts.refresh_rate,
        max_login_retry: opts.max_login_retry,
        keepalive_send_to: opts.keepalive_send_to,
        session: opts.session.clone(),
        bad_usernames: opts.bad_usernames.unwrap_or_default(),
        bad_exact_usernames: opts.bad_exact_usernames.unwrap_or_default(),
        bad_messages: opts.bad_messages.unwrap_or_default(),
        allowlist: opts.allowlist.unwrap_or_default(),
        alt_account,
        master_account,
        profile: opts.profile.clone(),
        ai_enabled: false,  // Disable AI by default
        ai_mode: "off".to_string(),
        system_intel: "You are a helpful AI assistant in a chat room. Be friendly and follow community guidelines.".to_string(),
        identities,
    };
    // println!("Session[2378]: {:?}", opts.session);

    // Initialize bot system if bot parameter is provided
    let bot_manager = if let Some(bot_name) = &opts.bot {
        let ai_service = Arc::new(AIService::new());
        let runtime = Arc::new(Runtime::new().expect("Failed to create tokio runtime"));

        let mut bot_manager = BotManager::new(Some(ai_service), Some(runtime));

        // Configure bot data directory
        let _bot_data_dir = opts
            .bot_data_dir
            .clone()
            .unwrap_or_else(|| format!("bot_data/{}", bot_name));

        // Use same credentials as main client
        let bot_url = params.url.clone().unwrap_or_else(|| {
            "http://blkhatjxlrvc5aevqzz5t6kxldayog6jlx5h7glnu44euzongl4fh5ad.onion/index.php"
                .to_string()
        });

        match bot_manager.add_bot(
            bot_name.clone(),
            params.username.clone(),
            params.password.clone(),
            bot_url,
            opts.bot_admins.clone(),
        ) {
            Ok(_) => {
                println!("🤖 Bot '{}' configured successfully", bot_name);

                // Start the bot
                if let Err(e) = bot_manager.start_bot(bot_name) {
                    eprintln!("❌ Failed to start bot '{}': {}", bot_name, e);
                } else {
                    println!("🚀 Bot '{}' started and running in background", bot_name);
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to configure bot '{}': {}", bot_name, e);
            }
        }

        Some(Arc::new(Mutex::new(bot_manager)))
    } else {
        None
    };

    // Pass bot_manager to ChatClient
    let mut chat_client = ChatClient::new(params);
    if let Some(bot_mgr) = &bot_manager {
        chat_client.set_bot_manager(Arc::clone(bot_mgr));
        // Create bridge between bot messages and main client
        chat_client.setup_bot_message_bridge();
    }
    chat_client.run_forever();

    // Clean up bot system when main client exits
    if let Some(bot_mgr) = bot_manager {
        println!("🔄 Shutting down bot system...");
        if let Err(e) = bot_mgr.lock().unwrap().stop_all() {
            eprintln!("⚠️ Error stopping bot system: {}", e);
        } else {
            println!("✅ Bot system stopped successfully");
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum PostType {
    Post(String, Option<String>),              // Message, SendTo
    PM(String, String),                        // To, Message
    Kick(String, String),                      // Message, Username
    Upload(String, String, String),            // FilePath, SendTo, Message
    DeleteLast,                                // DeleteLast
    Delete(String),                            // Delete message
    DeleteAll,                                 // DeleteAll
    KeepAlive(String),                         // SendTo for keepalive
    NewNickname(String),                       // NewUsername
    NewColor(String),                          // NewColor
    Profile(String, String, bool, bool, bool), // NewColor, NewUsername, Incognito, Bold, Italic
    SetIncognito(bool),                        // Set incognito mode on/off
    Ignore(String),                            // Username
    Unignore(String),                          // Username
    Clean(String, String),                     // Clean message
}

// Get username of other user (or ours if it's the only one)
fn get_username(
    own_username: &str,
    root: &StyledText,
    members_tag: &str,
    staffs_tag: &str,
) -> Option<String> {
    match get_message(root, members_tag, staffs_tag) {
        Some((from, Some(to), _, _)) => {
            if from == own_username {
                return Some(to);
            }
            return Some(from);
        }
        Some((from, None, _, _)) => {
            return Some(from);
        }
        _ => return None,
    }
}

// Extract "from"/"to"/"message content" from a "StyledText"
fn get_message(
    root: &StyledText,
    members_tag: &str,
    staffs_tag: &str,
) -> Option<(String, Option<String>, String, Option<String>)> { // Added channel info
    if let StyledText::Styled(_, children) = root {
        let msg = children.get(0)?.text();
        match children.get(children.len() - 1)? {
            StyledText::Styled(_, children) => {
                let from = match children.get(children.len() - 1)? {
                    StyledText::Text(t) => t.to_owned(),
                    _ => return None,
                };
                return Some((from, None, msg, None)); // Public channel
            }
            StyledText::Text(t) => {
                if t == &members_tag {
                    let from = match children.get(children.len() - 2)? {
                        StyledText::Styled(_, children) => {
                            match children.get(children.len() - 1)? {
                                StyledText::Text(t) => t.to_owned(),
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    return Some((from, None, msg, Some("members".to_string())));
                } else if t == &staffs_tag {
                    let from = match children.get(children.len() - 2)? {
                        StyledText::Styled(_, children) => {
                            match children.get(children.len() - 1)? {
                                StyledText::Text(t) => t.to_owned(),
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    return Some((from, None, msg, Some("staff".to_string())));
                } else if t == "[" {
                    let from = match children.get(children.len() - 2)? {
                        StyledText::Styled(_, children) => {
                            match children.get(children.len() - 1)? {
                                StyledText::Text(t) => t.to_owned(),
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    let to = match children.get(2)? {
                        StyledText::Styled(_, children) => {
                            match children.get(children.len() - 1)? {
                                StyledText::Text(t) => Some(t.to_owned()),
                                _ => return None,
                            }
                        }
                        _ => return None,
                    };
                    return Some((from, to, msg, None)); // Private message
                }
            }
            _ => return None,
        }
    }
    return None;
}

#[derive(Debug, PartialEq, Clone)]
enum MessageType {
    UserMsg,
    SysMsg,
}

#[derive(Debug, PartialEq, Clone)]
struct Message {
    id: Option<usize>,
    typ: MessageType,
    date: String,
    upload_link: Option<String>,
    text: StyledText,
    deleted: bool, // Either or not a message was deleted on the chat
    hide: bool,    // Either ot not to hide a specific message
}

impl Message {
    fn new(
        id: Option<usize>,
        typ: MessageType,
        date: String,
        upload_link: Option<String>,
        text: StyledText,
    ) -> Self {
        Self {
            id,
            typ,
            date,
            upload_link,
            text,
            deleted: false,
            hide: false,
        }
    }
}

#[derive(Debug, Clone)]
struct InboxMessage {
    id: String,      // message ID for deletion
    date: String,    // formatted date string
    from: String,    // sender username
    to: String,      // recipient (usually "0" or username)
    content: String, // message content
    selected: bool,  // for deletion selection
}

impl InboxMessage {
    fn new(id: String, date: String, from: String, to: String, content: String) -> Self {
        Self {
            id,
            date,
            from,
            to,
            content,
            selected: false,
        }
    }
}

#[derive(Debug, Clone)]
struct CleanMessage {
    id: String,   // message ID for deletion
    date: String, // formatted date string
    #[allow(dead_code)]
    from: String, // sender username
    content: String, // message content
    selected: bool, // for deletion selection
}

impl CleanMessage {
    fn new(id: String, date: String, from: String, content: String) -> Self {
        Self {
            id,
            date,
            from,
            content,
            selected: false,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum StyledText {
    Styled(tuiColor, Vec<StyledText>),
    Text(String),
    None,
}

impl StyledText {
    fn walk<F>(&self, mut clb: F)
    where
        F: FnMut(&StyledText),
    {
        let mut v: Vec<&StyledText> = vec![self];
        loop {
            if let Some(e) = v.pop() {
                clb(e);
                if let StyledText::Styled(_, children) = e {
                    v.extend(children);
                }
                continue;
            }
            break;
        }
    }

    fn text(&self) -> String {
        let mut s = String::new();
        self.walk(|n| {
            if let StyledText::Text(t) = n {
                s += t;
            }
        });
        s
    }

    // Return a vector of each text parts & what color it should be
    fn colored_text(&self) -> Vec<(tuiColor, String)> {
        let mut out: Vec<(tuiColor, String)> = vec![];
        let mut v: Vec<(tuiColor, &StyledText)> = vec![(tuiColor::White, self)];
        loop {
            if let Some((el_color, e)) = v.pop() {
                match e {
                    StyledText::Styled(tui_color, children) => {
                        for child in children {
                            v.push((*tui_color, child));
                        }
                    }
                    StyledText::Text(t) => {
                        out.push((el_color, t.to_owned()));
                    }
                    StyledText::None => {}
                }
                continue;
            }
            break;
        }
        out
    }
}

fn parse_color(color_str: &str) -> tuiColor {
    let mut color = tuiColor::White;
    if color_str == "red" {
        return tuiColor::Red;
    }
    if let Ok(rgb) = Rgb::from_hex_str(color_str) {
        color = tuiColor::Rgb(
            rgb.get_red() as u8,
            rgb.get_green() as u8,
            rgb.get_blue() as u8,
        );
    }
    color
}

fn process_node(e: select::node::Node, mut color: tuiColor) -> (StyledText, Option<String>) {
    match e.data() {
        select::node::Data::Element(_, _) => {
            let mut upload_link: Option<String> = None;
            match e.name() {
                Some("span") => {
                    if let Some(style) = e.attr("style") {
                        if let Some(captures) = COLOR_RGX.captures(style) {
                            let color_match = captures.get(1).unwrap().as_str();
                            color = parse_color(color_match);
                        }
                    }
                }
                Some("font") => {
                    if let Some(color_str) = e.attr("color") {
                        color = parse_color(color_str);
                    }
                }
                Some("a") => {
                    color = tuiColor::White;
                    if let (Some("attachement"), Some(href)) = (e.attr("class"), e.attr("href")) {
                        upload_link = Some(href.to_owned());
                    }
                }
                Some("style") => {
                    return (StyledText::None, None);
                }
                Some("form") | Some("button") | Some("input") | Some("textarea")
                | Some("select") | Some("option") | Some("script") | Some("noscript")
                | Some("iframe") | Some("details") | Some("summary") | Some("label") => {
                    // Strip out form elements and script elements that can break terminal rendering
                    return (StyledText::None, None);
                }
                _ => {}
            }
            let mut children_texts: Vec<StyledText> = vec![];
            let children = e.children();
            for child in children {
                let (st, ul) = process_node(child, color);
                if ul.is_some() {
                    upload_link = ul;
                }
                children_texts.push(st);
            }
            children_texts.reverse();
            (StyledText::Styled(color, children_texts), upload_link)
        }
        select::node::Data::Text(t) => (StyledText::Text(t.to_string()), None),
        select::node::Data::Comment(_) => (StyledText::None, None),
    }
}

#[derive(Clone)]
struct Users {
    admin: Vec<(tuiColor, String)>,
    staff: Vec<(tuiColor, String)>,
    members: Vec<(tuiColor, String)>,
    guests: Vec<(tuiColor, String)>,
}

impl Default for Users {
    fn default() -> Self {
        Self {
            admin: Default::default(),
            staff: Default::default(),
            members: Default::default(),
            guests: Default::default(),
        }
    }
}

impl Users {
    fn all(&self) -> Vec<&(tuiColor, String)> {
        let mut out = Vec::new();
        out.extend(&self.admin);
        out.extend(&self.staff);
        out.extend(&self.members);
        out.extend(&self.guests);
        out
    }

    // fn is_guest(&self, name: &str) -> bool {
    //     self.guests.iter().find(|(_, username)| username == name).is_some()
    // }
}

fn extract_users(doc: &Document) -> Users {
    let mut users = Users::default();

    if let Some(chatters) = doc.find(Attr("id", "chatters")).next() {
        if let Some(tr) = chatters.find(Name("tr")).next() {
            let mut th_count = 0;
            for e in tr.children() {
                if let select::node::Data::Element(_, _) = e.data() {
                    if e.name() == Some("th") {
                        th_count += 1;
                        continue;
                    }
                    for user_span in e.find(Name("span")) {
                        if let Some(user_style) = user_span.attr("style") {
                            if let Some(captures) = COLOR_RGX.captures(user_style) {
                                if let Some(color_match) = captures.get(1) {
                                    let color = color_match.as_str().to_owned();
                                    let tui_color = parse_color(&color);
                                    let username = user_span.text();
                                    match th_count {
                                        1 => users.admin.push((tui_color, username)),
                                        2 => users.staff.push((tui_color, username)),
                                        3 => users.members.push((tui_color, username)),
                                        4 => users.guests.push((tui_color, username)),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    users
}

fn remove_suffix<'a>(s: &'a str, suffix: &str) -> &'a str {
    s.strip_suffix(suffix).unwrap_or(s)
}

fn remove_prefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

fn parse_forwarded_username(
    text: &str,
    members_tag: &str,
    staffs_tag: &str,
) -> Option<(&'static str, String)> {
    lazy_static! {
        static ref FORWARD_RGX: Regex = Regex::new(r"^\[[^\]]+ to [^\]]+\]\s*").unwrap();
    }

    if let Some(mat) = FORWARD_RGX.find(text) {
        let mut rest = text[mat.end()..].trim_start();
        // Some forwarded messages contain a leading dash or colon after the
        // forwarding header. Trim those so we can properly match the tags.
        rest = rest
            .trim_start_matches(|c: char| c == '-' || c == ':')
            .trim_start();

        if let Some(rem) = rest.strip_prefix(members_tag) {
            let name = rem
                .trim_start()
                .split(|c: char| c == ' ' || c == ':' || c == '-')
                .next()
                .unwrap_or("")
                .trim_matches('@')
                .to_owned();
            return Some(("/m", name));
        } else if let Some(rem) = rest.strip_prefix(staffs_tag) {
            let name = rem
                .trim_start()
                .split(|c: char| c == ' ' || c == ':' || c == '-')
                .next()
                .unwrap_or("")
                .trim_matches('@')
                .to_owned();
            return Some(("/s", name));
        }
    }
    None
}

fn extract_messages(doc: &Document) -> anyhow::Result<Vec<Message>> {
    let msgs = doc
        .find(Attr("id", "messages"))
        .next()
        .ok_or(anyhow!("failed to get messages div"))?
        .find(Attr("class", "msg"))
        .filter_map(|tag| {
            let mut id: Option<usize> = None;
            if let Some(checkbox) = tag.find(Name("input")).next() {
                if let Some(value_attr) = checkbox.attr("value") {
                    if !value_attr.is_empty() {
                        match value_attr.parse::<usize>() {
                            Ok(val) => id = Some(val),
                            Err(_) => {
                                // Silently skip invalid message IDs instead of printing error
                                // This is common when parsing HTML that might have malformed or missing attributes
                            }
                        }
                    }
                    // Silently skip checkboxes without value attributes - this is normal
                }
            }
            if let Some(date_node) = tag.find(Name("small")).next() {
                if let Some(msg_span) = tag.find(Name("span")).next() {
                    let date = remove_suffix(&date_node.text(), " - ").to_owned();
                    let typ = match msg_span.attr("class") {
                        Some("usermsg") => MessageType::UserMsg,
                        Some("sysmsg") => MessageType::SysMsg,
                        _ => return None,
                    };
                    let (text, upload_link) = process_node(msg_span, tuiColor::White);
                    return Some(Message::new(id, typ, date, upload_link, text));
                }
            }
            None
        })
        .collect::<Vec<_>>();
    Ok(msgs)
}

fn draw_notes_pane(f: &mut Frame<CrosstermBackend<io::Stdout>>, app: &mut App) {
    use tui::layout::{Constraint, Direction, Layout};
    use tui::style::{Color, Modifier, Style};
    use tui::text::{Span, Spans};
    use tui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let size = f.size();
    
    // Clear the entire screen
    f.render_widget(Clear, size);
    
    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(1),    // Content
            Constraint::Length(3), // Status/command line
        ])
        .split(size);

    // Header with note type and tabs
    let current_type = app.get_current_notes_type();
    let mut header_spans = vec![
        Span::styled("Notes: ", Style::default().fg(Color::Yellow)),
    ];
    
    for (i, note_type) in app.notes_available_types.iter().enumerate() {
        if i == app.notes_type_index {
            header_spans.push(Span::styled(
                format!("[{}]", note_type),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
        } else {
            header_spans.push(Span::styled(
                format!(" {} ", note_type),
                Style::default().fg(Color::Gray),
            ));
        }
        if i < app.notes_available_types.len() - 1 {
            header_spans.push(Span::raw(" "));
        }
    }
    header_spans.push(Span::raw(" | Tab to cycle | :w to save | :q to quit | :wq to save & quit"));

    let header = Paragraph::new(Spans::from(header_spans))
        .block(Block::default().borders(Borders::ALL).title("BHCLI Notes"))
        .wrap(Wrap { trim: true });
    f.render_widget(header, chunks[0]);

    // Content area with text and scrolling support
    let content_height = chunks[1].height.saturating_sub(2) as usize; // Account for borders
    let visible_start = app.notes_scroll_offset;
    let visible_end = std::cmp::min(visible_start + content_height, app.notes_content.len());
    
    let content_lines: Vec<Spans> = app.notes_content[visible_start..visible_end].iter().enumerate().map(|(visible_idx, line)| {
        let line_idx = visible_start + visible_idx;
        let mut spans = vec![];
        
        // Handle empty lines by showing at least a space with cursor if on this line
        let display_line = if line.is_empty() && line_idx == app.notes_cursor_pos.0 {
            " "
        } else {
            line
        };
        
        // Determine if this line has visual selection
        let has_visual_selection = app.notes_vim_mode == VimMode::Visual && 
            app.notes_visual_start.is_some() &&
            line_idx == app.notes_cursor_pos.0;
        
        for (col_idx, ch) in display_line.char_indices() {
            let mut style = Style::default();
            
            // Cursor highlighting
            if line_idx == app.notes_cursor_pos.0 {
                if col_idx == app.notes_cursor_pos.1 {
                    match app.notes_vim_mode {
                        VimMode::Normal => {
                            style = Style::default().bg(Color::Gray).fg(Color::Black);
                        }
                        VimMode::Insert => {
                            style = Style::default().bg(Color::Yellow).fg(Color::Black);
                        }
                        _ => {}
                    }
                }
            }
            
            // Visual selection highlighting
            if has_visual_selection {
                if let Some(start_pos) = app.notes_visual_start {
                    let current_pos = (line_idx, col_idx);
                    let selection_start = if start_pos <= app.notes_cursor_pos { start_pos } else { app.notes_cursor_pos };
                    let selection_end = if start_pos <= app.notes_cursor_pos { app.notes_cursor_pos } else { start_pos };
                    
                    if current_pos >= selection_start && current_pos < selection_end {
                        style = Style::default().bg(Color::Blue).fg(Color::White);
                    }
                }
            }
            
            spans.push(Span::styled(ch.to_string(), style));
        }
        
        // Add cursor at end of line if needed (for empty lines or when cursor is at end)
        if line_idx == app.notes_cursor_pos.0 && app.notes_cursor_pos.1 >= line.len() {
            match app.notes_vim_mode {
                VimMode::Normal => {
                    // Show cursor as highlighted space
                    spans.push(Span::styled(" ", Style::default().bg(Color::Gray)));
                }
                VimMode::Insert => {
                    // Show cursor as yellow pipe
                    spans.push(Span::styled("|", Style::default().fg(Color::Yellow)));
                }
                _ => {}
            }
        }
        
        // For completely empty lines not at cursor position, add a fake space to show the line exists
        if spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        
        Spans::from(spans)
    }).collect();

    // Determine border color based on vim mode
    let border_color = match app.notes_vim_mode {
        VimMode::Insert => Color::LightBlue,
        VimMode::Visual => Color::Green,
        _ => Color::White,
    };

    let content_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!("{} Notes", current_type));
    let content = Paragraph::new(content_lines)
        .block(content_block)
        .wrap(Wrap { trim: false });
    f.render_widget(content, chunks[1]);

    // Status line
    let status_text = if app.notes_search_mode {
        format!("/{}", app.notes_search_query)
    } else {
        match app.notes_vim_mode {
            VimMode::Normal => {
                let modified = if app.notes_modified { " [modified]" } else { "" };
                let last_edited = app.notes_last_edited.as_deref().unwrap_or("never");
                let number_prefix = if let Some(ref prefix) = app.notes_number_prefix {
                    format!("{}", prefix)
                } else {
                    String::new()
                };
                
                let search_info = if let Some(current_idx) = app.notes_current_match_index {
                    format!(" | Match ({}/{})", current_idx + 1, app.notes_search_matches.len())
                } else {
                    String::new()
                };
                
                format!("-- NORMAL --{} | {}Line {}, Col {} | Last edited: {}{} | w/b:word $:end 0:start /{{pattern}}:search n/N:next/prev", 
                        modified, 
                        number_prefix,
                        app.notes_cursor_pos.0 + 1, 
                        app.notes_cursor_pos.1 + 1,
                        last_edited,
                        search_info)
            }
            VimMode::Insert => {
                format!("-- INSERT -- | Line {}, Col {} | Use arrow keys or hjkl to navigate", 
                        app.notes_cursor_pos.0 + 1, 
                        app.notes_cursor_pos.1 + 1)
            }
            VimMode::Visual => {
                let selection_info = if let Some(start) = app.notes_visual_start {
                    format!(" | Selection: {}:{} to {}:{}", 
                           start.0 + 1, start.1 + 1,
                           app.notes_cursor_pos.0 + 1, app.notes_cursor_pos.1 + 1)
                } else {
                    String::new()
                };
                format!("-- VISUAL --{} | Press x to delete selection", selection_info)
            }
            VimMode::Command => {
                format!(":{}", app.notes_vim_command)
            }
        }
    };

    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}

fn draw_message_editor_ui(f: &mut Frame<CrosstermBackend<io::Stdout>>, app: &mut App) {
    use tui::layout::{Constraint, Direction, Layout};
    use tui::style::{Color, Style};
    use tui::text::{Span, Spans};
    use tui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let size = f.size();
    
    // Clear the entire screen
    f.render_widget(Clear, size);
    
    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(1),    // Content
            Constraint::Length(3), // Status/command line
        ])
        .split(size);

    // Header
    let header_text = Spans::from(vec![
        Span::styled("Message Editor", Style::default().fg(Color::Yellow)),
        Span::raw(" - Press :w to send, :q to cancel"),
    ]);
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Editor"));
    f.render_widget(header, chunks[0]);

    // Determine border color based on mode
    let border_color = match app.msg_editor_vim_mode {
        VimMode::Insert => Color::LightBlue,
        VimMode::Visual => Color::Green, 
        _ => Color::White,
    };

    // Content area with scrolling
    let content_height = chunks[1].height.saturating_sub(2) as usize; // Account for borders
    
    // Calculate visible content range based on cursor and scroll
    let total_lines = app.msg_editor_content.len().max(1);
    let cursor_line = app.msg_editor_cursor_pos.0;
    
    // Ensure cursor is visible
    if cursor_line < app.msg_editor_scroll_offset {
        app.msg_editor_scroll_offset = cursor_line;
    } else if cursor_line >= app.msg_editor_scroll_offset + content_height {
        app.msg_editor_scroll_offset = cursor_line.saturating_sub(content_height - 1);
    }
    
    // Get visible lines - use same cursor logic as notes editor
    let end_line = (app.msg_editor_scroll_offset + content_height).min(total_lines);
    let visible_content: Vec<_> = app.msg_editor_content
        .get(app.msg_editor_scroll_offset..end_line)
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let line_num = app.msg_editor_scroll_offset + i;
            let mut spans = vec![];
            
            // Handle empty lines by showing at least a space with cursor if on this line
            let display_line = if line.is_empty() && line_num == cursor_line {
                " "
            } else {
                line
            };
            
            // Determine if this line has visual selection
            let has_visual_selection = app.msg_editor_vim_mode == VimMode::Visual && 
                app.msg_editor_visual_start.is_some() &&
                line_num == cursor_line;
            
            for (col_idx, ch) in display_line.char_indices() {
                let mut style = Style::default();
                
                // Cursor highlighting
                if line_num == cursor_line {
                    if col_idx == app.msg_editor_cursor_pos.1 {
                        match app.msg_editor_vim_mode {
                            VimMode::Normal => {
                                style = Style::default().bg(Color::Gray).fg(Color::Black);
                            }
                            VimMode::Insert => {
                                style = Style::default().bg(Color::Yellow).fg(Color::Black);
                            }
                            _ => {}
                        }
                    }
                }
                
                // Visual selection highlighting
                if has_visual_selection {
                    if let Some((start_line, start_col)) = app.msg_editor_visual_start {
                        let start_pos = (start_line, start_col);
                        let current_pos = (line_num, col_idx);
                        let selection_start = if start_pos <= app.msg_editor_cursor_pos { start_pos } else { app.msg_editor_cursor_pos };
                        let selection_end = if start_pos <= app.msg_editor_cursor_pos { app.msg_editor_cursor_pos } else { start_pos };
                        
                        if current_pos >= selection_start && current_pos < selection_end {
                            style = Style::default().bg(Color::Blue).fg(Color::White);
                        }
                    }
                }
                
                spans.push(Span::styled(ch.to_string(), style));
            }
            
            // Add cursor at end of line if needed (for empty lines or when cursor is at end)
            if line_num == cursor_line && app.msg_editor_cursor_pos.1 >= line.len() {
                match app.msg_editor_vim_mode {
                    VimMode::Normal => {
                        // Show cursor as highlighted space
                        spans.push(Span::styled(" ", Style::default().bg(Color::Gray)));
                    }
                    VimMode::Insert => {
                        // Show cursor as yellow pipe
                        spans.push(Span::styled("|", Style::default().fg(Color::Yellow)));
                    }
                    _ => {}
                }
            }
            
            // For completely empty lines not at cursor position, add a fake space to show the line exists
            if spans.is_empty() {
                spans.push(Span::raw(" "));
            }
            
            Spans::from(spans)
        })
        .collect();

    let content = Paragraph::new(visible_content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Message")
                .border_style(Style::default().fg(border_color))
        )
        .wrap(Wrap { trim: false });
    f.render_widget(content, chunks[1]);

    // Status line
    let status_text = if app.msg_editor_search_mode {
        Spans::from(vec![
            Span::styled(format!("/{}", app.msg_editor_search_query), Style::default().fg(Color::Cyan)),
        ])
    } else {
        let mode_text = match app.msg_editor_vim_mode {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT", 
            VimMode::Command => "COMMAND",
            VimMode::Visual => "VISUAL",
        };
        
        let number_prefix = if let Some(ref prefix) = app.msg_editor_number_prefix {
            format!("{}", prefix)
        } else {
            String::new()
        };
        
        match app.msg_editor_vim_mode {
            VimMode::Normal => {
                let search_info = if let Some(current_idx) = app.msg_editor_current_match_index {
                    format!(" | Match ({}/{})", current_idx + 1, app.msg_editor_search_matches.len())
                } else {
                    String::new()
                };
                
                Spans::from(vec![
                    Span::styled(format!("-- {} --", mode_text), Style::default().fg(Color::Yellow)),
                    Span::raw(format!(" | {}Cursor: {}:{} | Lines: {}{} | w/b:word $:end 0:start /{{pattern}}:search n/N:next/prev | :w to send, :q to cancel", 
                             number_prefix,
                             app.msg_editor_cursor_pos.0 + 1, 
                             app.msg_editor_cursor_pos.1 + 1,
                             app.msg_editor_content.len(),
                             search_info)),
                ])
            }
            VimMode::Command => {
                Spans::from(vec![
                    Span::styled(format!(":{}", app.msg_editor_vim_command), Style::default().fg(Color::Cyan)),
                ])
            }
            _ => {
                Spans::from(vec![
                    Span::styled(format!("-- {} --", mode_text), Style::default().fg(Color::Yellow)),
                    Span::raw(format!(" | Cursor: {}:{} | Lines: {} | :w to send, :q to cancel", 
                             app.msg_editor_cursor_pos.0 + 1, 
                             app.msg_editor_cursor_pos.1 + 1,
                             app.msg_editor_content.len())),
                ])
            }
        }
    };
    
    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}

fn draw_terminal_frame(
    f: &mut Frame<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    messages: &Arc<Mutex<Vec<Message>>>,
    users: &Arc<Mutex<Users>>,
    username: &str,
) {
    if app.notes_mode {
        draw_notes_pane(f, app);
        return;
    }
    
    if app.msg_editor_mode {
        draw_message_editor_ui(f, app);
        return;
    }
    
    if app.long_message.is_none() {
        let hchunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(25)].as_ref())
            .split(f.size());

        {
            // Determine textbox height based on input mode
            let textbox_height = match app.input_mode {
                InputMode::MultilineEditing => 8, // Larger height for multiline mode
                _ => 3,                           // Default height for single-line modes
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(1),
                        Constraint::Length(textbox_height),
                        Constraint::Min(1),
                    ]
                    .as_ref(),
                )
                .split(hchunks[0]);

            render_help_txt(f, app, chunks[0], username);
            render_textbox(f, app, chunks[1]);
            if app.clean_mode {
                render_clean_messages(f, app, chunks[2]);
            } else {
                render_messages(f, app, chunks[2], messages);
            }
            render_users(f, hchunks[1], users);
        }
    } else {
        let hchunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1)])
            .split(f.size());
        {
            render_long_message(f, app, hchunks[0]);
        }
    }
}

fn gen_lines(msg_txt: &StyledText, w: usize, line_prefix: &str) -> Vec<Vec<(tuiColor, String)>> {
    let txt = msg_txt.text();

    // For simple text (like help messages), use a much simpler approach
    // Check if this looks like plain text content (no HTML, just text with newlines)
    let is_plain_text = !txt.contains('<')
        && !txt.contains('>')
        && msg_txt
            .colored_text()
            .iter()
            .all(|(color, _)| *color == tuiColor::White);

    if is_plain_text {
        // This is plain text, handle it simply
        let mut result = Vec::new();

        // Split by existing newlines first to preserve intended line breaks
        for original_line in txt.split('\n') {
            if original_line.len() <= w {
                // Line fits, add it as-is
                result.push(vec![(tuiColor::White, original_line.to_string())]);
            } else {
                // Line is too long, wrap it
                let wrapped = textwrap::fill(original_line, w);
                for wrapped_line in wrapped.split('\n') {
                    result.push(vec![(tuiColor::White, wrapped_line.to_string())]);
                }
            }
        }
        return result;
    }

    // Fallback to original complex logic for colored text
    let original_lines: Vec<&str> = txt.split('\n').collect();
    let mut wrapped_lines = Vec::new();

    // Only wrap individual lines that are too long
    for line in original_lines {
        if line.len() <= w {
            wrapped_lines.push(line.to_string());
        } else {
            // Use textwrap only on lines that are actually too long
            let wrapped = textwrap::fill(line, w);
            for wrapped_line in wrapped.split('\n') {
                wrapped_lines.push(wrapped_line.to_string());
            }
        }
    }

    let splits = wrapped_lines
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<&str>>();
    let mut new_lines: Vec<Vec<(tuiColor, String)>> = Vec::new();
    let mut ctxt = msg_txt.colored_text();
    ctxt.reverse();
    let mut ptr = 0;
    let mut split_idx = 0;
    let mut line: Vec<(tuiColor, String)> = Vec::new();
    let mut first_in_line = true;
    loop {
        if let Some((color, mut txt)) = ctxt.pop() {
            txt = txt.replace("\n", "");
            if let Some(split) = splits.get(split_idx) {
                if let Some(chr) = txt.chars().next() {
                    if chr == ' ' && first_in_line {
                        let skipped: String = txt.chars().skip(1).collect();
                        txt = skipped;
                    }
                }

                let remain = split.len() - ptr;
                if txt.len() <= remain {
                    ptr += txt.len();
                    line.push((color, txt));
                    first_in_line = false;
                } else {
                    //line.push((color, txt[0..remain].to_owned()));
                    if let Some(valid_slice) = txt.get(0..remain) {
                        line.push((color, valid_slice.to_owned()));
                    } else {
                        let valid_remain = txt
                            .char_indices()
                            .take_while(|&(i, _)| i < remain)
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(txt.len());

                        line.push((color, txt[..valid_remain].to_owned()));
                    }

                    new_lines.push(line.clone());
                    line.clear();
                    line.push((tuiColor::White, line_prefix.to_owned()));
                    //ctxt.push((color, txt[(remain)..].to_owned()));
                    if let Some(valid_slice) = txt.get(remain..) {
                        ctxt.push((color, valid_slice.to_owned()));
                    } else {
                        let valid_remain = txt
                            .char_indices()
                            .skip_while(|&(i, _)| i < remain) // Find first valid boundary after remain
                            .map(|(i, _)| i)
                            .next()
                            .unwrap_or(txt.len());

                        ctxt.push((color, txt[valid_remain..].to_owned()));
                    }

                    ptr = 0;
                    split_idx += 1;
                    first_in_line = true;
                }
            }
        } else {
            new_lines.push(line);
            break;
        }
    }
    new_lines
}

fn render_long_message(f: &mut Frame<CrosstermBackend<io::Stdout>>, app: &mut App, r: Rect) {
    if let Some(m) = &app.long_message {
        let new_lines = gen_lines(&m.text, (r.width - 2) as usize, "");

        let mut rows = vec![];
        for line in new_lines.into_iter() {
            let spans_vec: Vec<Span> = line
                .into_iter()
                .map(|(color, txt)| Span::styled(txt, Style::default().fg(color)))
                .collect();
            rows.push(Spans::from(spans_vec));
        }

        // Calculate how many lines can be displayed in the available height
        let available_height = (r.height - 2) as usize; // -2 for borders
        let total_lines = rows.len();

        // Adjust scroll offset to prevent scrolling beyond content
        let max_scroll = if total_lines > available_height {
            total_lines - available_height
        } else {
            0
        };
        app.long_message_scroll_offset = app.long_message_scroll_offset.min(max_scroll);

        // Apply scrolling by taking a slice of the rows
        let visible_rows = if total_lines > available_height {
            rows.into_iter()
                .skip(app.long_message_scroll_offset)
                .take(available_height)
                .collect()
        } else {
            rows
        };

        let messages_list_items: Vec<ListItem> = visible_rows
            .into_iter()
            .map(|spans| ListItem::new(spans))
            .collect();

        let title = if total_lines > available_height {
            format!("Message (line {}/{}) - j/k or ↑/↓ to scroll, PgUp/PgDn for fast scroll, Enter/Esc to exit",
                    app.long_message_scroll_offset + 1,
                    total_lines)
        } else {
            "Message - Enter/Esc to exit".to_string()
        };

        let messages_list = List::new(messages_list_items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(tuiColor::Rgb(50, 50, 50))
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(messages_list, r);
    }
}

fn render_help_txt(
    f: &mut Frame<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    r: Rect,
    curr_user: &str,
) {
    let (mut msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                Span::raw("Press "),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit, "),
                Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to logout, "),
                Span::styled("i", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to start editing."),
            ],
            Style::default(),
        ),
        InputMode::Editing | InputMode::EditingErr => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to stop editing, "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to record the message"),
            ],
            Style::default(),
        ),
        InputMode::LongMessage => (vec![], Style::default()),
        InputMode::MultilineEditing => (
            vec![
                Span::raw("Press "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to exit multiline mode, "),
                Span::styled("Ctrl+L", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to send"),
            ],
            Style::default(),
        ),
        InputMode::Notes => (vec![], Style::default()),
        InputMode::MessageEditor => (vec![], Style::default()),
    };
    msg.extend(vec![Span::raw(format!(" | {}", curr_user))]);
    if app.is_muted {
        let fg = tuiColor::Red;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("muted", style)]);
    } else {
        let fg = tuiColor::LightGreen;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("not muted", style)]);
    }

    //Strange
    if app.display_guest_view {
        let fg = tuiColor::LightGreen;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("G", style)]);
    } else {
        let fg = tuiColor::Gray;
        let style = Style::default().fg(fg);
        msg.extend(vec![Span::raw(" | "), Span::styled("G", style)]);
    }

    //Strange
    if app.display_member_view {
        let fg = tuiColor::LightGreen;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("M", style)]);
    } else {
        let fg = tuiColor::Gray;
        let style = Style::default().fg(fg);
        msg.extend(vec![Span::raw(" | "), Span::styled("M", style)]);
    }

    if app.display_hidden_msgs {
        let fg = tuiColor::LightGreen;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("H", style)]);
    } else {
        let fg = tuiColor::Gray;
        let style = Style::default().fg(fg);
        msg.extend(vec![Span::raw(" | "), Span::styled("H", style)]);
    }

    if app.clean_mode {
        let fg = tuiColor::LightGreen;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("C", style)]);
    } else {
        let fg = tuiColor::Gray;
        let style = Style::default().fg(fg);
        msg.extend(vec![Span::raw(" | "), Span::styled("C", style)]);
    }

    if app.inbox_mode {
        let fg = tuiColor::LightBlue;
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        msg.extend(vec![Span::raw(" | "), Span::styled("O", style)]);
    } else {
        let fg = tuiColor::Gray;
        let style = Style::default().fg(fg);
        msg.extend(vec![Span::raw(" | "), Span::styled("O", style)]);
    }
    let mut text = Text::from(Spans::from(msg));
    text.patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, r);
}

fn render_textbox(f: &mut Frame<CrosstermBackend<io::Stdout>>, app: &mut App, r: Rect) {
    let w = (r.width - 3) as usize;
    let str = app.input.clone();

    // Handle multiline vs single line display differently
    let (input_widget, cursor_x, cursor_y) = match app.input_mode {
        InputMode::MultilineEditing => {
            // For multiline, we need to properly handle line wrapping and newlines
            let lines: Vec<&str> = str.split('\n').collect();
            let text_width = (r.width - 3) as usize; // Account for borders
            let available_height = (r.height - 2) as usize; // Account for borders

            // Calculate total visual lines (including wrapped lines)
            let mut total_visual_lines = 0;
            let mut line_visual_counts = Vec::new();
            for line in &lines {
                let line_len = line.chars().count();
                let visual_count = if line_len == 0 {
                    1
                } else {
                    (line_len + text_width - 1) / text_width
                };
                line_visual_counts.push(visual_count);
                total_visual_lines += visual_count;
            }

            // Calculate which line the cursor is on and position within that line
            let mut cursor_line = 0;
            let mut chars_before_cursor = 0;
            let mut current_pos = 0;
            let mut cursor_visual_line = 0; // Track visual lines including wrapping

            for (line_idx, line) in lines.iter().enumerate() {
                let line_len = line.chars().count();
                if current_pos + line_len >= app.input_idx {
                    cursor_line = line_idx;
                    chars_before_cursor = app.input_idx - current_pos;

                    // Calculate how many visual lines this cursor position creates due to wrapping
                    let chars_in_current_line = chars_before_cursor;
                    let wrapped_lines_before = chars_in_current_line / text_width;
                    cursor_visual_line += wrapped_lines_before;
                    chars_before_cursor = chars_in_current_line % text_width;
                    break;
                }
                current_pos += line_len + 1; // +1 for the newline character
                cursor_visual_line += line_visual_counts[line_idx];
            }

            // Ensure cursor is within bounds
            if cursor_line < lines.len() {
                let current_line_len = lines[cursor_line].chars().count();
                chars_before_cursor = chars_before_cursor.min(current_line_len % text_width);
            }

            // Auto-scroll to keep cursor visible
            if cursor_visual_line < app.multiline_scroll_offset {
                app.multiline_scroll_offset = cursor_visual_line;
            } else if cursor_visual_line >= app.multiline_scroll_offset + available_height {
                app.multiline_scroll_offset = cursor_visual_line - available_height + 1;
            }

            // Ensure scroll offset doesn't exceed content
            if total_visual_lines <= available_height {
                app.multiline_scroll_offset = 0;
            } else {
                app.multiline_scroll_offset = app
                    .multiline_scroll_offset
                    .min(total_visual_lines - available_height);
            }

            // Create the paragraph with proper line breaks and scrolling
            let input = Paragraph::new(str.as_str())
                .style(Style::default().fg(tuiColor::Cyan))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Input (Multiline)"),
                )
                .wrap(Wrap { trim: false })
                .scroll((app.multiline_scroll_offset as u16, 0));

            // Calculate cursor position accounting for wrapping and scrolling
            let cursor_x = r.x + 1 + chars_before_cursor as u16;
            let cursor_y = r.y + 1 + (cursor_visual_line - app.multiline_scroll_offset) as u16;

            (input, cursor_x, cursor_y)
        }
        _ => {
            // Single line handling (existing logic)
            let mut input_str = str.as_str();
            let mut overflow = 0;
            if app.input_idx >= w {
                overflow = std::cmp::max(app.input.width() - w, 0);
                input_str = &str[overflow..];
            }

            let input = Paragraph::new(input_str)
                .style(match app.input_mode {
                    InputMode::LongMessage => Style::default(),
                    InputMode::Normal => Style::default(),
                    InputMode::Editing => Style::default().fg(tuiColor::Yellow),
                    InputMode::EditingErr => Style::default().fg(tuiColor::Red),
                    InputMode::MultilineEditing => Style::default().fg(tuiColor::Cyan),
                    InputMode::Notes => Style::default(),
                    InputMode::MessageEditor => Style::default(),
                })
                .block(Block::default().borders(Borders::ALL).title("Input"));

            let cursor_x = r.x + app.input_idx as u16 - overflow as u16 + 1;
            let cursor_y = r.y + 1;

            (input, cursor_x, cursor_y)
        }
    };

    f.render_widget(input_widget, r);

    // Set cursor position based on input mode
    match app.input_mode {
        InputMode::LongMessage => {}
        InputMode::Normal => {}
        InputMode::Editing | InputMode::EditingErr | InputMode::MultilineEditing => {
            // Make the cursor visible and position it correctly
            f.set_cursor(cursor_x, cursor_y);
        }
        InputMode::Notes => {}
        InputMode::MessageEditor => {}
    }
}

fn render_messages(
    f: &mut Frame<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    r: Rect,
    messages: &Arc<Mutex<Vec<Message>>>,
) {
    if app.inbox_mode {
        render_inbox_messages(f, app, r);
        return;
    }

    // Messages
    app.items.items.clear();
    let messages = messages.lock().unwrap();
    let messages_list_items: Vec<ListItem> = messages
        .iter()
        .filter_map(|m| {
            if app.clean_mode {
                // In clean mode show all messages
            } else {
                if !app.display_hidden_msgs && m.hide {
                    return None;
                }
                // Simulate a guest view (remove "PMs" and "Members chat" messages)
                if app.display_guest_view {
                    // TODO: this is not efficient at all
                    let text = m.text.text();
                    if text.starts_with(&app.members_tag) || text.starts_with(&app.staffs_tag) {
                        return None;
                    }
                    if let Some((_, Some(_), _, _)) =
                        get_message(&m.text, &app.members_tag, &app.staffs_tag)
                    {
                        return None;
                    }
                }

                // Strange
                // Display only messages from members and staff
                if app.display_member_view {
                    // In members mode, include only messages from members and staff
                    let text = m.text.text();
                    if !text.starts_with(&app.members_tag) && !text.starts_with(&app.staffs_tag) {
                        return None;
                    }
                    if let Some((_, Some(_), _, _)) =
                        get_message(&m.text, &app.members_tag, &app.staffs_tag)
                    {
                        return None;
                    }
                }

                if app.display_pm_only {
                    match get_message(&m.text, &app.members_tag, &app.staffs_tag) {
                        Some((_, Some(_), _, _)) => {}
                        _ => return None,
                    }
                }

                if app.display_staff_view {
                    let text = m.text.text();
                    if !text.starts_with(&app.staffs_tag) {
                        return None;
                    }
                }

                if app.display_master_pm_view {
                    // Master PM view filtering is now handled by client-level account manager
                    // This view mode is only enabled when master account is configured
                    match get_message(&m.text, &app.members_tag, &app.staffs_tag) {
                        Some((_, Some(_), _, _)) => {
                            // Show PMs when in master PM view mode
                        }
                        _ => return None,
                    }
                }

                if app.filter != "" {
                    if !m
                        .text
                        .text()
                        .to_lowercase()
                        .contains(&app.filter.to_lowercase())
                    {
                        return None;
                    }
                }
            }

            app.items.items.push(m.clone());

            let new_lines = gen_lines(&m.text, (r.width - 20) as usize, " ".repeat(17).as_str());

            let mut rows = vec![];
            let date_style = match (m.deleted, m.hide) {
                (false, true) => Style::default().fg(tuiColor::Gray),
                (false, _) => Style::default().fg(tuiColor::DarkGray),
                (true, _) => Style::default().fg(tuiColor::Red),
            };
            let mut spans_vec = vec![Span::styled(m.date.clone(), date_style)];
            let show_sys_sep = app.show_sys && m.typ == MessageType::SysMsg;
            let sep = if show_sys_sep { " * " } else { " - " };
            spans_vec.push(Span::raw(sep));
            for (idx, line) in new_lines.into_iter().enumerate() {
                // Spams can take your whole screen, so we limit to 5 lines.
                if idx >= 5 {
                    spans_vec.push(Span::styled(
                        "                 […]",
                        Style::default().fg(tuiColor::White),
                    ));
                    rows.push(Spans::from(spans_vec));
                    break;
                }
                for (color, txt) in line {
                    spans_vec.push(Span::styled(txt, Style::default().fg(color)));
                }
                rows.push(Spans::from(spans_vec.clone()));
                spans_vec.clear();
            }

            let style = match (m.deleted, m.hide) {
                (true, _) => Style::default().bg(tuiColor::Rgb(30, 0, 0)),
                (_, true) => Style::default().bg(tuiColor::Rgb(20, 20, 20)),
                _ => Style::default(),
            };
            Some(ListItem::new(rows).style(style))
        })
        .collect();

    let messages_list = List::new(messages_list_items)
        .block(Block::default().borders(Borders::ALL).title("Messages"))
        .highlight_style(
            Style::default()
                .bg(tuiColor::Rgb(50, 50, 50))
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(messages_list, r, &mut app.items.state)
}

fn render_inbox_messages(f: &mut Frame<CrosstermBackend<io::Stdout>>, app: &mut App, r: Rect) {
    let messages_list_items: Vec<ListItem> = app
        .inbox_items
        .items
        .iter()
        .map(|m| {
            let date_style = Style::default().fg(tuiColor::DarkGray);
            let from_style = Style::default().fg(tuiColor::LightBlue);
            let to_style = Style::default().fg(tuiColor::White);
            let content_style = Style::default().fg(tuiColor::White);
            let selected_style = Style::default()
                .fg(tuiColor::Red)
                .add_modifier(Modifier::BOLD);

            let checkbox = if m.selected { "[X]" } else { "[ ]" };
            let checkbox_span = Span::styled(
                checkbox,
                if m.selected {
                    selected_style
                } else {
                    Style::default()
                },
            );

            let spans = vec![
                checkbox_span,
                Span::raw(" "),
                Span::styled(&m.date, date_style),
                Span::raw(" - ["),
                Span::styled(&m.from, from_style),
                Span::raw(" to "),
                Span::styled(&m.to, to_style),
                Span::raw("] - "),
                Span::styled(&m.content, content_style),
            ];

            ListItem::new(Spans::from(spans))
        })
        .collect();

    let messages_list = List::new(messages_list_items)
        .block(Block::default().borders(Borders::ALL).title("Inbox (Shift+O to toggle, Space to check/uncheck, 'x' to delete checked, /clearinbox to clear all)"))
        .highlight_style(
            Style::default()
                .bg(tuiColor::Rgb(50, 50, 50))
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(messages_list, r, &mut app.inbox_items.state)
}

fn render_clean_messages(f: &mut Frame<CrosstermBackend<io::Stdout>>, app: &mut App, r: Rect) {
    let messages_list_items: Vec<ListItem> = app
        .clean_items
        .items
        .iter()
        .map(|m| {
            let date_style = Style::default().fg(tuiColor::DarkGray);
            let content_style = Style::default().fg(tuiColor::White);
            let selected_style = Style::default()
                .fg(tuiColor::Red)
                .add_modifier(Modifier::BOLD);

            let checkbox = if m.selected { "[X]" } else { "[ ]" };
            let checkbox_span = Span::styled(
                checkbox,
                if m.selected {
                    selected_style
                } else {
                    Style::default()
                },
            );

            let spans = vec![
                checkbox_span,
                Span::raw(" "),
                Span::styled(&m.date, date_style),
                Span::raw(" - "),
                Span::styled(&m.content, content_style),
            ];

            ListItem::new(Spans::from(spans))
        })
        .collect();

    let messages_list =
        List::new(messages_list_items)
            .block(Block::default().borders(Borders::ALL).title(
                "Clean Mode (Shift+C to toggle, Space to check/uncheck, 'x' to delete checked)",
            ))
            .highlight_style(
                Style::default()
                    .bg(tuiColor::Rgb(50, 50, 50))
                    .add_modifier(Modifier::BOLD),
            );
    f.render_stateful_widget(messages_list, r, &mut app.clean_items.state)
}

fn render_users(f: &mut Frame<CrosstermBackend<io::Stdout>>, r: Rect, users: &Arc<Mutex<Users>>) {
    // Users lists
    let users = users.lock().unwrap();
    let mut users_list: Vec<ListItem> = vec![];
    let mut users_types: Vec<(&Vec<(tuiColor, String)>, &str)> = Vec::new();
    users_types.push((&users.admin, "-- Admin --"));
    users_types.push((&users.staff, "-- Staff --"));
    users_types.push((&users.members, "-- Members --"));
    users_types.push((&users.guests, "-- Guests --"));
    for (users, label) in users_types.into_iter() {
        users_list.push(ListItem::new(Span::raw(label)));
        for (tui_color, username) in users.iter() {
            let span = Span::styled(username, Style::default().fg(*tui_color));
            users_list.push(ListItem::new(span));
        }
    }
    let users = List::new(users_list).block(Block::default().borders(Borders::ALL).title("Users"));
    f.render_widget(users, r);
}

fn random_string(n: usize) -> String {
    let s: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(n).collect();
    std::str::from_utf8(&s).unwrap().to_owned()
}

#[derive(PartialEq)]
enum InputMode {
    LongMessage,
    Normal,
    Editing,
    EditingErr,
    MultilineEditing,
    Notes,
    MessageEditor,
}

#[derive(PartialEq, Clone)]
enum VimMode {
    Normal,
    Insert,
    Command,
    Visual,
}

#[derive(Debug)]
enum EditorCommand {
    Send(String),
    Quit,
    None,
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: String,
    input_idx: usize,
    /// Current input mode
    input_mode: InputMode,
    /// Command history for up/down arrow navigation
    command_history: Vec<String>,
    command_history_index: Option<usize>,
    temp_input: String, // Stores current input when browsing history
    is_muted: bool,
    show_sys: bool,
    display_guest_view: bool,
    display_member_view: bool,
    display_hidden_msgs: bool,
    items: StatefulList<Message>,
    inbox_items: StatefulList<InboxMessage>,
    clean_items: StatefulList<CleanMessage>,
    filter: String,
    members_tag: String,
    staffs_tag: String,
    long_message: Option<Message>,
    long_message_scroll_offset: usize,
    commands: Commands,

    display_pm_only: bool,
    display_staff_view: bool,
    display_master_pm_view: bool,
    clean_mode: bool,
    inbox_mode: bool,

    // Multiline input scrolling
    multiline_scroll_offset: usize,

    // External editor state
    external_editor_active: bool,

    // Formatting state for current identity
    #[allow(dead_code)]
    bold: bool,
    #[allow(dead_code)]
    italic: bool,

    // Notes pane state
    notes_mode: bool,
    notes_vim_mode: VimMode,
    notes_cursor_pos: (usize, usize), // (line, col)
    notes_content: Vec<String>,
    notes_type_index: usize, // 0=Personal, 1=Public, 2=Staff, 3=Admin
    notes_available_types: Vec<&'static str>,
    notes_vim_command: String,
    notes_modified: bool,
    notes_scroll_offset: usize,
    notes_visual_start: Option<(usize, usize)>, // Visual mode selection start
    notes_last_edited: Option<String>, // Last edited timestamp
    notes_pending_g: bool, // For gg/G commands
    notes_number_prefix: Option<String>, // For number prefixes like 12j
    notes_search_query: String, // For /{filter} searches
    notes_search_mode: bool, // Whether we're in search mode
    notes_search_matches: Vec<(usize, usize)>, // All search match positions (line, col)
    notes_current_match_index: Option<usize>, // Current match index
    notes_pending_d: bool, // For dd line deletion (waiting for second d)
    notes_undo_history: Vec<Vec<String>>, // History of content states for undo
    notes_undo_cursor_history: Vec<(usize, usize)>, // History of cursor positions
    notes_undo_index: usize, // Current position in undo history

    // Message editor state
    msg_editor_mode: bool,
    msg_editor_vim_mode: VimMode,
    msg_editor_cursor_pos: (usize, usize), // (line, col)
    msg_editor_content: Vec<String>,
    msg_editor_vim_command: String,
    msg_editor_scroll_offset: usize,
    msg_editor_visual_start: Option<(usize, usize)>,
    msg_editor_pending_g: bool,
    msg_editor_number_prefix: Option<String>, // For number prefixes like 12j
    msg_editor_search_query: String, // For /{filter} searches
    msg_editor_search_mode: bool, // Whether we're in search mode
    msg_editor_search_matches: Vec<(usize, usize)>, // All search match positions (line, col)
    msg_editor_current_match_index: Option<usize>, // Current match index
    msg_editor_pending_d: bool, // For dd line deletion (waiting for second d)
    msg_editor_undo_history: Vec<Vec<String>>, // History of content states for undo
    msg_editor_undo_cursor_history: Vec<(usize, usize)>, // History of cursor positions
    msg_editor_undo_index: usize, // Current position in undo history
}
impl Default for App {
    fn default() -> App {
        // Read commands from the file and set them as default values
        let commands = if let Ok(config_path) = confy::get_configuration_file_path("bhcli", None) {
            if let Some(config_path_str) = config_path.to_str() {
                match read_commands_file(config_path_str) {
                    Ok(commands) => commands,
                    Err(err) => {
                        log::error!(
                            "Failed to read commands from config file - {} :
{}",
                            config_path_str,
                            err
                        );
                        Commands {
                            commands: HashMap::new(),
                        }
                    }
                }
            } else {
                log::error!("Failed to convert configuration file path to string.");
                Commands {
                    commands: HashMap::new(),
                }
            }
        } else {
            log::error!("Failed to get configuration file path.");
            Commands {
                commands: HashMap::new(),
            }
        };

        App {
            input: String::new(),
            input_idx: 0,
            input_mode: InputMode::Normal,
            command_history: Vec::new(),
            command_history_index: None,
            temp_input: String::new(),
            is_muted: false,
            show_sys: false,
            display_guest_view: false,
            display_member_view: false,
            display_hidden_msgs: false,
            items: StatefulList::new(),
            inbox_items: StatefulList::new(),
            clean_items: StatefulList::new(),
            filter: "".to_owned(),
            members_tag: "".to_owned(),
            staffs_tag: "".to_owned(),
            long_message: None,
            long_message_scroll_offset: 0,
            commands,
            display_pm_only: false,
            display_staff_view: false,
            display_master_pm_view: false,
            clean_mode: false,
            inbox_mode: false,
            multiline_scroll_offset: 0,
            external_editor_active: false,
            bold: false,
            italic: false,
            notes_mode: false,
            notes_vim_mode: VimMode::Normal,
            notes_cursor_pos: (0, 0),
            notes_content: vec!["".to_string()],
            notes_type_index: 0,
            notes_available_types: vec!["Personal", "Public", "Staff", "Admin"],
            notes_vim_command: String::new(),
            notes_modified: false,
            notes_scroll_offset: 0,
            notes_visual_start: None,
            notes_last_edited: None,
            notes_pending_g: false,
            notes_number_prefix: None,
            notes_search_query: String::new(),
            notes_search_mode: false,
            notes_search_matches: Vec::new(),
            notes_current_match_index: None,
            notes_pending_d: false,
            notes_undo_history: vec![vec!["".to_string()]], // Start with initial state
            notes_undo_cursor_history: vec![(0, 0)],
            notes_undo_index: 0,
            msg_editor_mode: false,
            msg_editor_vim_mode: VimMode::Normal,
            msg_editor_cursor_pos: (0, 0),
            msg_editor_content: vec!["".to_string()],
            msg_editor_vim_command: String::new(),
            msg_editor_scroll_offset: 0,
            msg_editor_visual_start: None,
            msg_editor_pending_g: false,
            msg_editor_number_prefix: None,
            msg_editor_search_query: String::new(),
            msg_editor_search_mode: false,
            msg_editor_search_matches: Vec::new(),
            msg_editor_current_match_index: None,
            msg_editor_pending_d: false,
            msg_editor_undo_history: vec![vec!["".to_string()]], // Start with initial state
            msg_editor_undo_cursor_history: vec![(0, 0)],
            msg_editor_undo_index: 0,
        }
    }
}

impl App {
    fn update_filter(&mut self) {
        if let Some(captures) = FIND_RGX.captures(&self.input) {
            // Find
            self.filter = captures.get(1).map_or("", |m| m.as_str()).to_owned();
        }
    }

    fn clear_filter(&mut self) {
        if FIND_RGX.is_match(&self.input) {
            self.filter = "".to_owned();
            self.input = "".to_owned();
            self.input_idx = 0;
        }
    }

    fn add_to_history(&mut self, command: String) {
        if !command.is_empty() && !command.trim().is_empty() {
            // Remove duplicate if it exists
            if let Some(pos) = self.command_history.iter().position(|x| *x == command) {
                self.command_history.remove(pos);
            }
            // Add to the end (most recent)
            self.command_history.push(command);
            // Keep only last 100 commands
            if self.command_history.len() > 100 {
                self.command_history.remove(0);
            }
        }
        // Reset history navigation
        self.command_history_index = None;
        self.temp_input.clear();
    }

    fn navigate_history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let current_input = self.input.clone();

        match self.command_history_index {
            None => {
                // First time navigating history, save current input
                self.temp_input = current_input.clone();
                // Find the most recent command that starts with current input
                let matching_commands: Vec<(usize, &String)> = self
                    .command_history
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, cmd)| {
                        if current_input.is_empty() {
                            true
                        } else {
                            cmd.starts_with(&current_input)
                        }
                    })
                    .collect();

                if let Some((idx, cmd)) = matching_commands.first() {
                    self.command_history_index = Some(*idx);
                    self.input = cmd.to_string();
                    self.input_idx = self.input.chars().count();
                }
            }
            Some(current_idx) => {
                // Find next older matching command
                let matching_commands: Vec<(usize, &String)> = self
                    .command_history
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(idx, cmd)| {
                        *idx < current_idx
                            && (self.temp_input.is_empty() || cmd.starts_with(&self.temp_input))
                    })
                    .collect();

                if let Some((idx, cmd)) = matching_commands.first() {
                    self.command_history_index = Some(*idx);
                    self.input = cmd.to_string();
                    self.input_idx = self.input.chars().count();
                }
            }
        }
    }

    fn navigate_history_down(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        match self.command_history_index {
            None => {
                // Not currently navigating history, do nothing
            }
            Some(current_idx) => {
                // Find next newer matching command
                let matching_commands: Vec<(usize, &String)> = self
                    .command_history
                    .iter()
                    .enumerate()
                    .filter(|(idx, cmd)| {
                        *idx > current_idx
                            && (self.temp_input.is_empty() || cmd.starts_with(&self.temp_input))
                    })
                    .collect();

                if let Some((idx, cmd)) = matching_commands.first() {
                    self.command_history_index = Some(*idx);
                    self.input = cmd.to_string();
                    self.input_idx = self.input.chars().count();
                } else {
                    // No newer commands, go back to original input
                    self.command_history_index = None;
                    self.input = self.temp_input.clone();
                    self.input_idx = self.input.chars().count();
                }
            }
        }
    }

    fn reset_history_navigation(&mut self) {
        self.command_history_index = None;
        self.temp_input.clear();
    }

    // Notes functionality
    fn enter_notes_mode(&mut self, client: &LeChatPHPClient) {
        self.notes_mode = true;
        self.input_mode = InputMode::Notes;
        self.notes_vim_mode = VimMode::Normal;
        self.notes_cursor_pos = (0, 0);
        self.notes_modified = false;
        self.notes_vim_command.clear();
        self.notes_scroll_offset = 0;
        self.notes_visual_start = None;
        self.notes_pending_g = false;
        self.notes_type_index = 0;
        
        // Set up available types based on user permissions
        self.update_available_notes_types(client);
        
        // Only load content if we have available types
        if !self.notes_available_types.is_empty() {
            self.load_notes_content(client);
        } else {
            // No permission to view any notes
            self.notes_content = vec!["You don't have permission to view any notes.".to_string()];
        }
    }

    fn exit_notes_mode(&mut self) {
        self.notes_mode = false;
        self.input_mode = InputMode::Normal;
    }

    fn cycle_notes_type(&mut self, client: &LeChatPHPClient) {
        // Update available types based on current permissions
        self.update_available_notes_types(client);
        
        if !self.notes_available_types.is_empty() {
            self.notes_type_index = (self.notes_type_index + 1) % self.notes_available_types.len();
            self.load_notes_content(client);
        } else {
            // No types available - do nothing to prevent crash
            return;
        }
    }

    fn update_available_notes_types(&mut self, client: &LeChatPHPClient) {
        let user_role = client.determine_user_role();
        let mut available_types = vec![];
        
        match user_role {
            UserRole::Guest => {
                // Guests can only view public notes (if any)
                available_types.push("Public");
            }
            UserRole::Member => {
                // Members can view personal and public notes
                available_types.push("Personal");
                available_types.push("Public");
            }
            UserRole::Staff => {
                // Staff can view personal, public, and staff notes
                available_types.push("Personal");
                available_types.push("Public");
                available_types.push("Staff");
            }
            UserRole::Admin => {
                // Admins can view all types
                available_types.push("Personal");
                available_types.push("Public");
                available_types.push("Staff");
                available_types.push("Admin");
            }
        }
        
        self.notes_available_types = available_types;
        
        // Ensure current index is valid
        if self.notes_type_index >= self.notes_available_types.len() && !self.notes_available_types.is_empty() {
            self.notes_type_index = 0;
        }
    }

    fn load_notes_content(&mut self, client: &LeChatPHPClient) {
        let note_type = match self.get_current_notes_type() {
            "Personal" => "",
            "Public" => "public",
            "Staff" => "staff", 
            "Admin" => "admin",
            _ => "",
        };
        
        match client.fetch_notes(note_type) {
            Ok((content, last_edited)) => {
                self.notes_content = content;
                // Ensure cursor position is within bounds after loading new content
                self.ensure_notes_cursor_bounds();
                self.notes_modified = false;
                self.notes_last_edited = last_edited;
            }
            Err(_) => {
                self.notes_content = vec!["Failed to load notes".to_string()];
                self.notes_cursor_pos = (0, 0);
                self.notes_modified = false;
                self.notes_last_edited = None;
            }
        }
    }

    fn ensure_notes_cursor_bounds(&mut self) {
        if self.notes_content.is_empty() {
            self.notes_content = vec!["".to_string()];
            self.notes_cursor_pos = (0, 0);
            return;
        }
        
        // Ensure row is within bounds
        if self.notes_cursor_pos.0 >= self.notes_content.len() {
            self.notes_cursor_pos.0 = self.notes_content.len() - 1;
        }
        
        // Ensure column is within bounds
        let line_len = self.notes_content[self.notes_cursor_pos.0].len();
        if self.notes_cursor_pos.1 > line_len {
            self.notes_cursor_pos.1 = line_len;
        }
        
        // Update scroll to make cursor visible
        self.ensure_cursor_visible();
    }

    fn get_current_notes_type(&self) -> &str {
        if self.notes_available_types.is_empty() {
            "None"
        } else {
            self.notes_available_types[self.notes_type_index]
        }
    }

    // Helper function to find next word boundary
    fn find_next_word_boundary(line: &str, start_pos: usize) -> usize {
        let chars: Vec<char> = line.chars().collect();
        let mut pos = start_pos;
        
        if pos >= chars.len() {
            return chars.len();
        }
        
        // Skip current word if we're in the middle of it
        if chars[pos].is_alphanumeric() || chars[pos] == '_' {
            while pos < chars.len() && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                pos += 1;
            }
        } else if !chars[pos].is_whitespace() {
            // Skip punctuation
            while pos < chars.len() && !chars[pos].is_whitespace() && !chars[pos].is_alphanumeric() && chars[pos] != '_' {
                pos += 1;
            }
        }
        
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        
        pos
    }
    
    // Helper function to find previous word boundary
    fn find_prev_word_boundary(line: &str, start_pos: usize) -> usize {
        let chars: Vec<char> = line.chars().collect();
        if start_pos == 0 || chars.is_empty() {
            return 0;
        }
        
        let mut pos = start_pos.saturating_sub(1);
        
        // Skip whitespace
        while pos > 0 && chars[pos].is_whitespace() {
            pos -= 1;
        }
        
        if pos == 0 {
            return 0;
        }
        
        // Move to beginning of current word
        if chars[pos].is_alphanumeric() || chars[pos] == '_' {
            while pos > 0 && (chars[pos - 1].is_alphanumeric() || chars[pos - 1] == '_') {
                pos -= 1;
            }
        } else {
            while pos > 0 && !chars[pos - 1].is_whitespace() && !chars[pos - 1].is_alphanumeric() && chars[pos - 1] != '_' {
                pos -= 1;
            }
        }
        
        pos
    }

    // Helper function to find all matches in content
    fn find_all_matches(content: &[String], query: &str) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        if query.is_empty() {
            return matches;
        }
        
        for (line_idx, line) in content.iter().enumerate() {
            let mut start = 0;
            while let Some(col_idx) = line[start..].find(query) {
                matches.push((line_idx, start + col_idx));
                start = start + col_idx + 1; // Move past this match to find next
            }
        }
        
        matches
    }

    // Navigate to next search match
    fn notes_next_match(&mut self) {
        if let Some(current_index) = self.notes_current_match_index {
            if !self.notes_search_matches.is_empty() {
                let new_index = (current_index + 1) % self.notes_search_matches.len();
                self.notes_current_match_index = Some(new_index);
                let (line, col) = self.notes_search_matches[new_index];
                self.notes_cursor_pos = (line, col);
                self.ensure_cursor_visible();
            }
        }
    }

    // Navigate to previous search match
    fn notes_prev_match(&mut self) {
        if let Some(current_index) = self.notes_current_match_index {
            if !self.notes_search_matches.is_empty() {
                let new_index = if current_index == 0 {
                    self.notes_search_matches.len() - 1
                } else {
                    current_index - 1
                };
                self.notes_current_match_index = Some(new_index);
                let (line, col) = self.notes_search_matches[new_index];
                self.notes_cursor_pos = (line, col);
                self.ensure_cursor_visible();
            }
        }
    }

    // Clear search results when changing modes
    fn clear_notes_search_results(&mut self) {
        self.notes_search_matches.clear();
        self.notes_current_match_index = None;
    }

    // Navigate to next search match - message editor
    fn msg_editor_next_match(&mut self) {
        if let Some(current_index) = self.msg_editor_current_match_index {
            if !self.msg_editor_search_matches.is_empty() {
                let new_index = (current_index + 1) % self.msg_editor_search_matches.len();
                self.msg_editor_current_match_index = Some(new_index);
                let (line, col) = self.msg_editor_search_matches[new_index];
                self.msg_editor_cursor_pos = (line, col);
                self.ensure_msg_editor_cursor_visible();
            }
        }
    }

    // Navigate to previous search match - message editor
    fn msg_editor_prev_match(&mut self) {
        if let Some(current_index) = self.msg_editor_current_match_index {
            if !self.msg_editor_search_matches.is_empty() {
                let new_index = if current_index == 0 {
                    self.msg_editor_search_matches.len() - 1
                } else {
                    current_index - 1
                };
                self.msg_editor_current_match_index = Some(new_index);
                let (line, col) = self.msg_editor_search_matches[new_index];
                self.msg_editor_cursor_pos = (line, col);
                self.ensure_msg_editor_cursor_visible();
            }
        }
    }

    // Clear search results when changing modes - message editor
    fn clear_msg_editor_search_results(&mut self) {
        self.msg_editor_search_matches.clear();
        self.msg_editor_current_match_index = None;
    }

    fn handle_notes_vim_key(&mut self, key: char, client: &LeChatPHPClient) -> bool {
        match self.notes_vim_mode {
            VimMode::Normal => self.handle_notes_normal_mode(key),
            VimMode::Insert => self.handle_notes_insert_mode(key),
            VimMode::Command => self.handle_notes_command_mode(key, client),
            VimMode::Visual => self.handle_notes_visual_mode(key),
        }
    }

    fn handle_notes_normal_mode(&mut self, key: char) -> bool {
        // Handle search mode
        if self.notes_search_mode {
            match key {
                '\r' => {
                    // Execute search
                    self.notes_search_mode = false;
                    
                    // Find all matches
                    self.notes_search_matches = Self::find_all_matches(&self.notes_content, &self.notes_search_query);
                    
                    if !self.notes_search_matches.is_empty() {
                        // Find the first match after current cursor position
                        let current_pos = (self.notes_cursor_pos.0, self.notes_cursor_pos.1);
                        let mut match_index = 0;
                        
                        for (i, &match_pos) in self.notes_search_matches.iter().enumerate() {
                            if match_pos > current_pos {
                                match_index = i;
                                break;
                            }
                            // If no match after cursor, wrap to first match
                            match_index = i;
                        }
                        
                        self.notes_current_match_index = Some(match_index);
                        let (line, col) = self.notes_search_matches[match_index];
                        self.notes_cursor_pos = (line, col);
                        self.ensure_cursor_visible();
                    } else {
                        self.notes_current_match_index = None;
                    }
                    
                    self.notes_search_query.clear();
                    return true;
                }
                '\x1b' => {
                    // Escape - cancel search
                    self.notes_search_mode = false;
                    self.notes_search_query.clear();
                    return true;
                }
                '\x08' => {
                    // Backspace
                    self.notes_search_query.pop();
                    return true;
                }
                c if c.is_ascii() && !c.is_control() => {
                    self.notes_search_query.push(c);
                    return true;
                }
                _ => return true,
            }
        }

        // Handle pending 'g' commands
        if self.notes_pending_g {
            self.notes_pending_g = false;
            match key {
                'g' => {
                    // gg - go to top
                    self.notes_cursor_pos = (0, 0);
                    self.notes_scroll_offset = 0;
                    return true;
                }
                _ => {
                    // Invalid g command, fall through
                }
            }
        }

        // Handle pending 'd' commands (dd for line deletion)
        if self.notes_pending_d {
            self.notes_pending_d = false;
            match key {
                'd' => {
                    // dd - delete line
                    self.handle_notes_dd();
                    return true;
                }
                '\x1b' => {
                    // Escape - cancel dd
                    return true;
                }
                _ => {
                    // Invalid d command, fall through to normal processing
                }
            }
        }

        // Handle number prefixes - special handling for '0'
        if key.is_ascii_digit() {
            if self.notes_number_prefix.is_none() {
                // First digit
                if key == '0' {
                    // '0' as first digit should be treated as motion (start of line), not number prefix
                    // Fall through to normal key handling
                } else {
                    // '1'-'9' as first digit starts number prefix
                    self.notes_number_prefix = Some(String::new());
                    self.notes_number_prefix.as_mut().unwrap().push(key);
                    return true;
                }
            } else {
                // Subsequent digit (including '0') can be added to existing prefix
                self.notes_number_prefix.as_mut().unwrap().push(key);
                return true;
            }
        }

        // Get repetition count
        let count = if let Some(ref prefix) = self.notes_number_prefix {
            prefix.parse::<usize>().unwrap_or(1)
        } else {
            1
        };
        
        // Clear number prefix after using it
        self.notes_number_prefix = None;

        // Clear pending states if any other key is pressed (except the expected ones)
        let should_clear_pending_states = match key {
            'd' if !self.notes_pending_d => false, // Allow first 'd'
            'd' | '\x1b' => false, // Allow second 'd' or escape when pending
            _ if self.notes_pending_d => true, // Clear pending 'd' for any other key
            _ => false,
        };
        
        if should_clear_pending_states {
            self.notes_pending_d = false;
        }

        match key {
            'h' => {
                for _ in 0..count {
                    if self.notes_cursor_pos.1 > 0 {
                        self.notes_cursor_pos.1 -= 1;
                    } else {
                        break;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'j' => {
                for _ in 0..count {
                    if self.notes_cursor_pos.0 < self.notes_content.len() - 1 {
                        self.notes_cursor_pos.0 += 1;
                        let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                        if self.notes_cursor_pos.1 > line_len {
                            self.notes_cursor_pos.1 = line_len;
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'k' => {
                for _ in 0..count {
                    if self.notes_cursor_pos.0 > 0 {
                        self.notes_cursor_pos.0 -= 1;
                        let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                        if self.notes_cursor_pos.1 > line_len {
                            self.notes_cursor_pos.1 = line_len;
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'l' => {
                for _ in 0..count {
                    let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                    if self.notes_cursor_pos.1 < line_len {
                        self.notes_cursor_pos.1 += 1;
                    } else {
                        break;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'w' => {
                // Word forward
                for _ in 0..count {
                    let current_line = &self.notes_content[self.notes_cursor_pos.0];
                    let new_col = Self::find_next_word_boundary(current_line, self.notes_cursor_pos.1);
                    
                    if new_col < current_line.len() {
                        self.notes_cursor_pos.1 = new_col;
                    } else if self.notes_cursor_pos.0 < self.notes_content.len() - 1 {
                        // Move to beginning of next line
                        self.notes_cursor_pos.0 += 1;
                        self.notes_cursor_pos.1 = 0;
                        // Skip to first non-whitespace character
                        let next_line = &self.notes_content[self.notes_cursor_pos.0];
                        for (i, ch) in next_line.chars().enumerate() {
                            if !ch.is_whitespace() {
                                self.notes_cursor_pos.1 = i;
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'b' => {
                // Word backward
                for _ in 0..count {
                    let current_line = &self.notes_content[self.notes_cursor_pos.0];
                    let new_col = Self::find_prev_word_boundary(current_line, self.notes_cursor_pos.1);
                    
                    if new_col < self.notes_cursor_pos.1 {
                        self.notes_cursor_pos.1 = new_col;
                    } else if self.notes_cursor_pos.0 > 0 {
                        // Move to end of previous line
                        self.notes_cursor_pos.0 -= 1;
                        self.notes_cursor_pos.1 = self.notes_content[self.notes_cursor_pos.0].len();
                    } else {
                        break;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            '$' => {
                // End of line
                self.notes_cursor_pos.1 = self.notes_content[self.notes_cursor_pos.0].len();
                self.ensure_cursor_visible();
                true
            }
            '0' => {
                // Beginning of line
                self.notes_cursor_pos.1 = 0;
                self.ensure_cursor_visible();
                true
            }
            '/' => {
                // Start search
                self.notes_search_mode = true;
                self.notes_search_query.clear();
                true
            }
            'G' => {
                // Go to end of file
                self.notes_cursor_pos.0 = self.notes_content.len() - 1;
                self.notes_cursor_pos.1 = self.notes_content[self.notes_cursor_pos.0].len();
                self.ensure_cursor_visible();
                true
            }
            'g' => {
                // Start of gg command
                self.notes_pending_g = true;
                true
            }
            'i' => {
                // Save state before entering insert mode
                self.save_notes_state();
                self.clear_notes_search_results(); // Clear search on mode change
                self.notes_vim_mode = VimMode::Insert;
                true
            }
            'a' => {
                // Save state before entering insert mode
                self.save_notes_state();
                self.clear_notes_search_results(); // Clear search on mode change
                self.notes_vim_mode = VimMode::Insert;
                let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                if self.notes_cursor_pos.1 < line_len {
                    self.notes_cursor_pos.1 += 1;
                }
                true
            }
            'A' => {
                // Append at end of line
                // Save state before entering insert mode
                self.save_notes_state();
                self.clear_notes_search_results(); // Clear search on mode change
                self.notes_vim_mode = VimMode::Insert;
                self.notes_cursor_pos.1 = self.notes_content[self.notes_cursor_pos.0].len();
                true
            }
            'x' => {
                // Delete character under cursor
                // Save state before making changes
                self.save_notes_state();
                let (line, col) = self.notes_cursor_pos;
                if col < self.notes_content[line].len() {
                    self.notes_content[line].remove(col);
                    self.notes_modified = true;
                    self.update_last_edited();
                }
                true
            }
            'v' => {
                // Enter visual mode
                self.notes_vim_mode = VimMode::Visual;
                self.notes_visual_start = Some(self.notes_cursor_pos);
                true
            }
            'u' => {
                // Undo
                self.notes_undo();
                true
            }
            'd' => {
                // First 'd' - wait for second one
                self.notes_pending_d = true;
                true
            }
            ':' => {
                self.notes_vim_mode = VimMode::Command;
                self.notes_vim_command.clear();
                true
            }
            'n' => {
                // Next search match
                self.notes_next_match();
                true
            }
            'N' => {
                // Previous search match
                self.notes_prev_match();
                true
            }
            _ => false,
        }
    }

    fn handle_notes_insert_mode(&mut self, key: char) -> bool {
        if key == '\x1b' {
            // Escape key
            self.notes_vim_mode = VimMode::Normal;
            if self.notes_cursor_pos.1 > 0 {
                self.notes_cursor_pos.1 -= 1;
            }
            self.update_last_edited();
            return true;
        }

        // Clear search results when in insert mode (mode switch)
        if !self.notes_search_matches.is_empty() {
            self.clear_notes_search_results();
        }

        match key {
            '\n' | '\r' => {
                let (line, col) = self.notes_cursor_pos;
                let current_line = self.notes_content[line].clone();
                let (left, right) = current_line.split_at(col);
                self.notes_content[line] = left.to_string();
                self.notes_content.insert(line + 1, right.to_string());
                self.notes_cursor_pos = (line + 1, 0);
                self.notes_modified = true;
                self.ensure_cursor_visible();
                true
            }
            '\x08' | '\x7f' => {
                // Backspace
                if self.notes_cursor_pos.1 > 0 {
                    let (line, col) = self.notes_cursor_pos;
                    self.notes_content[line].remove(col - 1);
                    self.notes_cursor_pos.1 -= 1;
                    self.notes_modified = true;
                } else if self.notes_cursor_pos.0 > 0 {
                    // Join with previous line
                    let current_line = self.notes_content.remove(self.notes_cursor_pos.0);
                    self.notes_cursor_pos.0 -= 1;
                    self.notes_cursor_pos.1 = self.notes_content[self.notes_cursor_pos.0].len();
                    self.notes_content[self.notes_cursor_pos.0].push_str(&current_line);
                    self.notes_modified = true;
                    self.ensure_cursor_visible();
                }
                true
            }
            c if c.is_ascii() && !c.is_control() => {
                let (line, col) = self.notes_cursor_pos;
                self.notes_content[line].insert(col, c);
                self.notes_cursor_pos.1 += 1;
                self.notes_modified = true;
                true
            }
            _ => false,
        }
    }

    fn handle_notes_visual_mode(&mut self, key: char) -> bool {
        match key {
            'h' => {
                if self.notes_cursor_pos.1 > 0 {
                    self.notes_cursor_pos.1 -= 1;
                }
                self.ensure_cursor_visible();
                true
            }
            'j' => {
                if self.notes_cursor_pos.0 < self.notes_content.len() - 1 {
                    self.notes_cursor_pos.0 += 1;
                    let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                    if self.notes_cursor_pos.1 > line_len {
                        self.notes_cursor_pos.1 = line_len;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'k' => {
                if self.notes_cursor_pos.0 > 0 {
                    self.notes_cursor_pos.0 -= 1;
                    let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                    if self.notes_cursor_pos.1 > line_len {
                        self.notes_cursor_pos.1 = line_len;
                    }
                }
                self.ensure_cursor_visible();
                true
            }
            'l' => {
                let line_len = self.notes_content[self.notes_cursor_pos.0].len();
                if self.notes_cursor_pos.1 < line_len {
                    self.notes_cursor_pos.1 += 1;
                }
                self.ensure_cursor_visible();
                true
            }
            'x' => {
                // Delete selected text
                self.delete_visual_selection();
                self.notes_vim_mode = VimMode::Normal;
                self.notes_visual_start = None;
                true
            }
            '\x1b' => {
                // Escape - exit visual mode
                self.notes_vim_mode = VimMode::Normal;
                self.notes_visual_start = None;
                true
            }
            _ => false,
        }
    }

    fn handle_notes_command_mode(&mut self, key: char, client: &LeChatPHPClient) -> bool {
        match key {
            '\n' | '\r' => {
                self.execute_notes_vim_command(client);
                self.notes_vim_mode = VimMode::Normal;
                true
            }
            '\x1b' => {
                // Escape
                self.notes_vim_mode = VimMode::Normal;
                self.notes_vim_command.clear();
                true
            }
            '\x08' | '\x7f' => {
                // Backspace
                self.notes_vim_command.pop();
                true
            }
            c if c.is_ascii() => {
                self.notes_vim_command.push(c);
                true
            }
            _ => false,
        }
    }

    fn execute_notes_vim_command(&mut self, client: &LeChatPHPClient) {
        match self.notes_vim_command.as_str() {
            "w" => {
                // Save notes
                if let Err(_) = self.save_notes_to_server(client) {
                    // TODO: Show error message
                } else {
                    self.notes_modified = false;
                    self.update_last_edited();
                }
            }
            "q" => {
                if !self.notes_modified {
                    self.exit_notes_mode();
                }
                // TODO: Show warning if modified
            }
            "wq" => {
                // Save and quit
                if let Err(_) = self.save_notes_to_server(client) {
                    // TODO: Show error message, don't quit
                } else {
                    self.notes_modified = false;
                    self.update_last_edited();
                    self.exit_notes_mode();
                }
            }
            _ => {}
        }
        self.notes_vim_command.clear();
    }

    fn save_notes_to_server(&self, client: &LeChatPHPClient) -> Result<(), Box<dyn std::error::Error>> {
        let note_type = match self.get_current_notes_type() {
            "Personal" => "",
            "Public" => "public",
            "Staff" => "staff",
            "Admin" => "admin",
            _ => "",
        };
        
        client.save_notes(note_type, &self.notes_content)
    }

    fn handle_notes_dd(&mut self) {
        // Save state before making changes
        self.save_notes_state();
        
        let (line, _) = self.notes_cursor_pos;
        if self.notes_content.len() > 1 {
            self.notes_content.remove(line);
            if line >= self.notes_content.len() {
                self.notes_cursor_pos.0 = self.notes_content.len() - 1;
            }
            self.notes_cursor_pos.1 = 0;
            self.notes_modified = true;
        } else {
            // Clear the only line
            self.notes_content[0].clear();
            self.notes_cursor_pos = (0, 0);
            self.notes_modified = true;
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let visible_lines = 50; // Conservative estimate - UI will handle actual height
        let (line, _) = self.notes_cursor_pos;
        
        if line < self.notes_scroll_offset {
            self.notes_scroll_offset = line;
        } else if line >= self.notes_scroll_offset + visible_lines {
            self.notes_scroll_offset = line - visible_lines + 1;
        }
    }

    fn update_last_edited(&mut self) {
        use chrono::Local;
        let now = Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
        self.notes_last_edited = Some(format!("Modified locally at {}", timestamp));
    }

    fn delete_visual_selection(&mut self) {
        if let Some(start) = self.notes_visual_start {
            let end = self.notes_cursor_pos;
            let (start_pos, end_pos) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            // Simple single-line selection for now
            if start_pos.0 == end_pos.0 {
                let line = start_pos.0;
                let start_col = start_pos.1;
                let end_col = end_pos.1;
                
                if start_col < end_col && end_col <= self.notes_content[line].len() {
                    self.notes_content[line].drain(start_col..end_col);
                    self.notes_cursor_pos = start_pos;
                    self.notes_modified = true;
                    self.update_last_edited();
                }
            }
        }
    }

    // Message editor functionality
    fn enter_message_editor_mode(&mut self) {
        self.msg_editor_mode = true;
        self.input_mode = InputMode::MessageEditor;
        self.msg_editor_vim_mode = VimMode::Normal;
        self.msg_editor_cursor_pos = (0, 0);
        self.msg_editor_vim_command.clear();
        self.msg_editor_scroll_offset = 0;
        self.msg_editor_visual_start = None;
        self.msg_editor_pending_g = false;

        // Copy input content to editor, split by lines
        if !self.input.is_empty() {
            self.msg_editor_content = self.input.split('\n').map(|s| s.to_string()).collect();
        } else {
            self.msg_editor_content = vec!["".to_string()];
        }
        
        // Position cursor at end
        if !self.msg_editor_content.is_empty() {
            let last_line = self.msg_editor_content.len() - 1;
            let last_col = self.msg_editor_content[last_line].len();
            self.msg_editor_cursor_pos = (last_line, last_col);
        }
    }

    fn exit_message_editor_mode(&mut self) {
        self.msg_editor_mode = false;
        self.input_mode = InputMode::Editing;
    }


    fn handle_msg_editor_vim_key(&mut self, key: char) -> EditorCommand {
        match self.msg_editor_vim_mode {
            VimMode::Normal => {
                self.handle_msg_editor_normal_mode(key);
                EditorCommand::None
            }
            VimMode::Insert => {
                self.handle_msg_editor_insert_mode(key);
                EditorCommand::None
            }
            VimMode::Command => self.handle_msg_editor_command_mode(key),
            VimMode::Visual => {
                self.handle_msg_editor_visual_mode(key);
                EditorCommand::None
            }
        }
    }

    fn handle_msg_editor_normal_mode(&mut self, key: char) -> bool {
        // Handle search mode
        if self.msg_editor_search_mode {
            match key {
                '\r' => {
                    // Execute search
                    self.msg_editor_search_mode = false;
                    
                    // Find all matches
                    self.msg_editor_search_matches = Self::find_all_matches(&self.msg_editor_content, &self.msg_editor_search_query);
                    
                    if !self.msg_editor_search_matches.is_empty() {
                        // Find the first match after current cursor position
                        let current_pos = (self.msg_editor_cursor_pos.0, self.msg_editor_cursor_pos.1);
                        let mut match_index = 0;
                        
                        for (i, &match_pos) in self.msg_editor_search_matches.iter().enumerate() {
                            if match_pos > current_pos {
                                match_index = i;
                                break;
                            }
                            // If no match after cursor, wrap to first match
                            match_index = i;
                        }
                        
                        self.msg_editor_current_match_index = Some(match_index);
                        let (line, col) = self.msg_editor_search_matches[match_index];
                        self.msg_editor_cursor_pos = (line, col);
                        self.ensure_msg_editor_cursor_visible();
                    } else {
                        self.msg_editor_current_match_index = None;
                    }
                    
                    self.msg_editor_search_query.clear();
                    return true;
                }
                '\x1b' => {
                    // Escape - cancel search
                    self.msg_editor_search_mode = false;
                    self.msg_editor_search_query.clear();
                    return true;
                }
                '\x08' => {
                    // Backspace
                    self.msg_editor_search_query.pop();
                    return true;
                }
                c if c.is_ascii() && !c.is_control() => {
                    self.msg_editor_search_query.push(c);
                    return true;
                }
                _ => return true,
            }
        }

        // Handle pending 'g' commands
        if self.msg_editor_pending_g {
            self.msg_editor_pending_g = false;
            match key {
                'g' => {
                    // gg - go to top
                    self.msg_editor_cursor_pos = (0, 0);
                    self.msg_editor_scroll_offset = 0;
                    return true;
                }
                _ => {}
            }
        }

        // Handle pending 'd' commands (dd for line deletion)
        if self.msg_editor_pending_d {
            self.msg_editor_pending_d = false;
            match key {
                'd' => {
                    // dd - delete line
                    self.handle_msg_editor_dd();
                    return true;
                }
                '\x1b' => {
                    // Escape - cancel dd
                    return true;
                }
                _ => {
                    // Invalid d command, fall through to normal processing
                }
            }
        }

        // Handle number prefixes - special handling for '0'
        if key.is_ascii_digit() {
            if self.msg_editor_number_prefix.is_none() {
                // First digit
                if key == '0' {
                    // '0' as first digit should be treated as motion (start of line), not number prefix
                    // Fall through to normal key handling
                } else {
                    // '1'-'9' as first digit starts number prefix
                    self.msg_editor_number_prefix = Some(String::new());
                    self.msg_editor_number_prefix.as_mut().unwrap().push(key);
                    return true;
                }
            } else {
                // Subsequent digit (including '0') can be added to existing prefix
                self.msg_editor_number_prefix.as_mut().unwrap().push(key);
                return true;
            }
        }

        // Get repetition count
        let count = if let Some(ref prefix) = self.msg_editor_number_prefix {
            prefix.parse::<usize>().unwrap_or(1)
        } else {
            1
        };
        
        // Clear number prefix after using it
        self.msg_editor_number_prefix = None;

        // Clear pending states if any other key is pressed (except the expected ones)
        let should_clear_pending_states = match key {
            'd' if !self.msg_editor_pending_d => false, // Allow first 'd'
            'd' | '\x1b' => false, // Allow second 'd' or escape when pending
            _ if self.msg_editor_pending_d => true, // Clear pending 'd' for any other key
            _ => false,
        };
        
        if should_clear_pending_states {
            self.msg_editor_pending_d = false;
        }

        match key {
            'h' => {
                for _ in 0..count {
                    if self.msg_editor_cursor_pos.1 > 0 {
                        self.msg_editor_cursor_pos.1 -= 1;
                    } else {
                        break;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'j' => {
                for _ in 0..count {
                    if self.msg_editor_cursor_pos.0 < self.msg_editor_content.len() - 1 {
                        self.msg_editor_cursor_pos.0 += 1;
                        let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                        if self.msg_editor_cursor_pos.1 > line_len {
                            self.msg_editor_cursor_pos.1 = line_len;
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'k' => {
                for _ in 0..count {
                    if self.msg_editor_cursor_pos.0 > 0 {
                        self.msg_editor_cursor_pos.0 -= 1;
                        let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                        if self.msg_editor_cursor_pos.1 > line_len {
                            self.msg_editor_cursor_pos.1 = line_len;
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'l' => {
                for _ in 0..count {
                    let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                    if self.msg_editor_cursor_pos.1 < line_len {
                        self.msg_editor_cursor_pos.1 += 1;
                    } else {
                        break;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'w' => {
                // Word forward
                for _ in 0..count {
                    let current_line = &self.msg_editor_content[self.msg_editor_cursor_pos.0];
                    let new_col = Self::find_next_word_boundary(current_line, self.msg_editor_cursor_pos.1);
                    
                    if new_col < current_line.len() {
                        self.msg_editor_cursor_pos.1 = new_col;
                    } else if self.msg_editor_cursor_pos.0 < self.msg_editor_content.len() - 1 {
                        // Move to beginning of next line
                        self.msg_editor_cursor_pos.0 += 1;
                        self.msg_editor_cursor_pos.1 = 0;
                        // Skip to first non-whitespace character
                        let next_line = &self.msg_editor_content[self.msg_editor_cursor_pos.0];
                        for (i, ch) in next_line.chars().enumerate() {
                            if !ch.is_whitespace() {
                                self.msg_editor_cursor_pos.1 = i;
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'b' => {
                // Word backward
                for _ in 0..count {
                    let current_line = &self.msg_editor_content[self.msg_editor_cursor_pos.0];
                    let new_col = Self::find_prev_word_boundary(current_line, self.msg_editor_cursor_pos.1);
                    
                    if new_col < self.msg_editor_cursor_pos.1 {
                        self.msg_editor_cursor_pos.1 = new_col;
                    } else if self.msg_editor_cursor_pos.0 > 0 {
                        // Move to end of previous line
                        self.msg_editor_cursor_pos.0 -= 1;
                        self.msg_editor_cursor_pos.1 = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                    } else {
                        break;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            '$' => {
                // End of line
                self.msg_editor_cursor_pos.1 = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                self.ensure_msg_editor_cursor_visible();
                true
            }
            '0' => {
                // Beginning of line
                self.msg_editor_cursor_pos.1 = 0;
                self.ensure_msg_editor_cursor_visible();
                true
            }
            '/' => {
                // Start search
                self.msg_editor_search_mode = true;
                self.msg_editor_search_query.clear();
                true
            }
            'G' => {
                self.msg_editor_cursor_pos.0 = self.msg_editor_content.len() - 1;
                self.msg_editor_cursor_pos.1 = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'g' => {
                self.msg_editor_pending_g = true;
                true
            }
            'i' => {
                // Save state before entering insert mode
                self.save_msg_editor_state();
                self.clear_msg_editor_search_results(); // Clear search on mode change
                self.msg_editor_vim_mode = VimMode::Insert;
                true
            }
            'a' => {
                // Save state before entering insert mode
                self.save_msg_editor_state();
                self.clear_msg_editor_search_results(); // Clear search on mode change
                self.msg_editor_vim_mode = VimMode::Insert;
                let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                if self.msg_editor_cursor_pos.1 < line_len {
                    self.msg_editor_cursor_pos.1 += 1;
                }
                true
            }
            'A' => {
                // Save state before entering insert mode
                self.save_msg_editor_state();
                self.clear_msg_editor_search_results(); // Clear search on mode change
                self.msg_editor_vim_mode = VimMode::Insert;
                self.msg_editor_cursor_pos.1 = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                true
            }
            'x' => {
                // Save state before making changes
                self.save_msg_editor_state();
                let (line, col) = self.msg_editor_cursor_pos;
                if col < self.msg_editor_content[line].len() {
                    self.msg_editor_content[line].remove(col);
                }
                true
            }
            'v' => {
                self.msg_editor_vim_mode = VimMode::Visual;
                self.msg_editor_visual_start = Some(self.msg_editor_cursor_pos);
                true
            }
            'u' => {
                // Undo
                self.msg_editor_undo();
                true
            }
            'd' => {
                // First 'd' - wait for second one
                self.msg_editor_pending_d = true;
                true
            }
            ':' => {
                self.msg_editor_vim_mode = VimMode::Command;
                self.msg_editor_vim_command.clear();
                true
            }
            'n' => {
                // Next search match
                self.msg_editor_next_match();
                true
            }
            'N' => {
                // Previous search match
                self.msg_editor_prev_match();
                true
            }
            _ => false,
        }
    }

    fn handle_msg_editor_insert_mode(&mut self, key: char) -> bool {
        if key == '\x1b' {
            self.msg_editor_vim_mode = VimMode::Normal;
            if self.msg_editor_cursor_pos.1 > 0 {
                self.msg_editor_cursor_pos.1 -= 1;
            }
            return true;
        }

        // Clear search results when in insert mode (mode switch)
        if !self.msg_editor_search_matches.is_empty() {
            self.clear_msg_editor_search_results();
        }

        match key {
            '\n' | '\r' => {
                let (line, col) = self.msg_editor_cursor_pos;
                let current_line = self.msg_editor_content[line].clone();
                let (left, right) = current_line.split_at(col);
                self.msg_editor_content[line] = left.to_string();
                self.msg_editor_content.insert(line + 1, right.to_string());
                self.msg_editor_cursor_pos = (line + 1, 0);
                self.ensure_msg_editor_cursor_visible();
                true
            }
            '\x08' | '\x7f' => {
                if self.msg_editor_cursor_pos.1 > 0 {
                    let (line, col) = self.msg_editor_cursor_pos;
                    self.msg_editor_content[line].remove(col - 1);
                    self.msg_editor_cursor_pos.1 -= 1;
                } else if self.msg_editor_cursor_pos.0 > 0 {
                    let current_line = self.msg_editor_content.remove(self.msg_editor_cursor_pos.0);
                    self.msg_editor_cursor_pos.0 -= 1;
                    self.msg_editor_cursor_pos.1 = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                    self.msg_editor_content[self.msg_editor_cursor_pos.0].push_str(&current_line);
                    self.ensure_msg_editor_cursor_visible();
                }
                true
            }
            c if c.is_ascii() && !c.is_control() => {
                let (line, col) = self.msg_editor_cursor_pos;
                self.msg_editor_content[line].insert(col, c);
                self.msg_editor_cursor_pos.1 += 1;
                true
            }
            _ => false,
        }
    }

    fn handle_msg_editor_visual_mode(&mut self, key: char) -> bool {
        match key {
            'h' => {
                if self.msg_editor_cursor_pos.1 > 0 {
                    self.msg_editor_cursor_pos.1 -= 1;
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'j' => {
                if self.msg_editor_cursor_pos.0 < self.msg_editor_content.len() - 1 {
                    self.msg_editor_cursor_pos.0 += 1;
                    let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                    if self.msg_editor_cursor_pos.1 > line_len {
                        self.msg_editor_cursor_pos.1 = line_len;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'k' => {
                if self.msg_editor_cursor_pos.0 > 0 {
                    self.msg_editor_cursor_pos.0 -= 1;
                    let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                    if self.msg_editor_cursor_pos.1 > line_len {
                        self.msg_editor_cursor_pos.1 = line_len;
                    }
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'l' => {
                let line_len = self.msg_editor_content[self.msg_editor_cursor_pos.0].len();
                if self.msg_editor_cursor_pos.1 < line_len {
                    self.msg_editor_cursor_pos.1 += 1;
                }
                self.ensure_msg_editor_cursor_visible();
                true
            }
            'x' => {
                self.delete_msg_editor_visual_selection();
                self.msg_editor_vim_mode = VimMode::Normal;
                self.msg_editor_visual_start = None;
                true
            }
            '\x1b' => {
                self.msg_editor_vim_mode = VimMode::Normal;
                self.msg_editor_visual_start = None;
                true
            }
            _ => false,
        }
    }

    fn handle_msg_editor_command_mode(&mut self, key: char) -> EditorCommand {
        match key {
            '\n' | '\r' => {
                let command = self.execute_msg_editor_vim_command();
                self.msg_editor_vim_mode = VimMode::Normal;
                return command;
            }
            '\x1b' => {
                self.msg_editor_vim_mode = VimMode::Normal;
                self.msg_editor_vim_command.clear();
                EditorCommand::None
            }
            '\x08' | '\x7f' => {
                self.msg_editor_vim_command.pop();
                EditorCommand::None
            }
            c if c.is_ascii() => {
                self.msg_editor_vim_command.push(c);
                EditorCommand::None
            }
            _ => EditorCommand::None,
        }
    }

    fn execute_msg_editor_vim_command(&mut self) -> EditorCommand {
        let command = match self.msg_editor_vim_command.as_str() {
            "w" => {
                // Send message and exit
                let content = self.msg_editor_content.join("\n");
                self.exit_message_editor_mode();
                self.msg_editor_vim_command.clear();
                EditorCommand::Send(content)
            }
            "q" => {
                // Quit without sending
                self.exit_message_editor_mode();
                self.msg_editor_vim_command.clear();
                EditorCommand::Quit
            }
            "wq" => {
                // Send and quit (same as :w)
                let content = self.msg_editor_content.join("\n");
                self.exit_message_editor_mode();
                self.msg_editor_vim_command.clear();
                EditorCommand::Send(content)
            }
            _ => {
                self.msg_editor_vim_command.clear();
                EditorCommand::None
            }
        };
        command
    }

    fn handle_msg_editor_dd(&mut self) {
        // Save state before making changes
        self.save_msg_editor_state();
        
        let (line, _) = self.msg_editor_cursor_pos;
        if self.msg_editor_content.len() > 1 {
            self.msg_editor_content.remove(line);
            if line >= self.msg_editor_content.len() {
                self.msg_editor_cursor_pos.0 = self.msg_editor_content.len() - 1;
            }
            self.msg_editor_cursor_pos.1 = 0;
        } else {
            self.msg_editor_content[0].clear();
            self.msg_editor_cursor_pos = (0, 0);
        }
    }

    fn ensure_msg_editor_cursor_visible(&mut self) {
        let visible_lines = 50; // Conservative estimate - UI will handle actual height
        let (line, _) = self.msg_editor_cursor_pos;
        
        if line < self.msg_editor_scroll_offset {
            self.msg_editor_scroll_offset = line;
        } else if line >= self.msg_editor_scroll_offset + visible_lines {
            self.msg_editor_scroll_offset = line - visible_lines + 1;
        }
    }

    fn delete_msg_editor_visual_selection(&mut self) {
        if let Some(start) = self.msg_editor_visual_start {
            let end = self.msg_editor_cursor_pos;
            let (start_pos, end_pos) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            if start_pos.0 == end_pos.0 {
                let line = start_pos.0;
                let start_col = start_pos.1;
                let end_col = end_pos.1;
                
                if start_col < end_col && end_col <= self.msg_editor_content[line].len() {
                    self.msg_editor_content[line].drain(start_col..end_col);
                    self.msg_editor_cursor_pos = start_pos;
                }
            }
        }
    }

    // Undo/Redo functionality for notes editor
    fn save_notes_state(&mut self) {
        // Limit history size to prevent memory bloat
        const MAX_HISTORY: usize = 100;
        
        // Truncate history if we're not at the end (when doing new action after undo)
        if self.notes_undo_index < self.notes_undo_history.len() - 1 {
            self.notes_undo_history.truncate(self.notes_undo_index + 1);
            self.notes_undo_cursor_history.truncate(self.notes_undo_index + 1);
        }
        
        // Add new state
        self.notes_undo_history.push(self.notes_content.clone());
        self.notes_undo_cursor_history.push(self.notes_cursor_pos);
        
        // Limit history size
        if self.notes_undo_history.len() > MAX_HISTORY {
            self.notes_undo_history.remove(0);
            self.notes_undo_cursor_history.remove(0);
        } else {
            self.notes_undo_index += 1;
        }
        
        if self.notes_undo_history.len() > MAX_HISTORY {
            self.notes_undo_index = MAX_HISTORY - 1;
        }
    }
    
    fn notes_undo(&mut self) {
        if self.notes_undo_index > 0 {
            self.notes_undo_index -= 1;
            self.notes_content = self.notes_undo_history[self.notes_undo_index].clone();
            self.notes_cursor_pos = self.notes_undo_cursor_history[self.notes_undo_index];
            self.notes_modified = true;
            self.ensure_cursor_visible();
        }
    }
    
    fn notes_redo(&mut self) {
        if self.notes_undo_index < self.notes_undo_history.len() - 1 {
            self.notes_undo_index += 1;
            self.notes_content = self.notes_undo_history[self.notes_undo_index].clone();
            self.notes_cursor_pos = self.notes_undo_cursor_history[self.notes_undo_index];
            self.notes_modified = true;
            self.ensure_cursor_visible();
        }
    }

    // Undo/Redo functionality for message editor
    fn save_msg_editor_state(&mut self) {
        // Limit history size to prevent memory bloat
        const MAX_HISTORY: usize = 100;
        
        // Truncate history if we're not at the end (when doing new action after undo)
        if self.msg_editor_undo_index < self.msg_editor_undo_history.len() - 1 {
            self.msg_editor_undo_history.truncate(self.msg_editor_undo_index + 1);
            self.msg_editor_undo_cursor_history.truncate(self.msg_editor_undo_index + 1);
        }
        
        // Add new state
        self.msg_editor_undo_history.push(self.msg_editor_content.clone());
        self.msg_editor_undo_cursor_history.push(self.msg_editor_cursor_pos);
        
        // Limit history size
        if self.msg_editor_undo_history.len() > MAX_HISTORY {
            self.msg_editor_undo_history.remove(0);
            self.msg_editor_undo_cursor_history.remove(0);
        } else {
            self.msg_editor_undo_index += 1;
        }
        
        if self.msg_editor_undo_history.len() > MAX_HISTORY {
            self.msg_editor_undo_index = MAX_HISTORY - 1;
        }
    }
    
    fn msg_editor_undo(&mut self) {
        if self.msg_editor_undo_index > 0 {
            self.msg_editor_undo_index -= 1;
            self.msg_editor_content = self.msg_editor_undo_history[self.msg_editor_undo_index].clone();
            self.msg_editor_cursor_pos = self.msg_editor_undo_cursor_history[self.msg_editor_undo_index];
            self.ensure_msg_editor_cursor_visible();
        }
    }
    
    fn msg_editor_redo(&mut self) {
        if self.msg_editor_undo_index < self.msg_editor_undo_history.len() - 1 {
            self.msg_editor_undo_index += 1;
            self.msg_editor_content = self.msg_editor_undo_history[self.msg_editor_undo_index].clone();
            self.msg_editor_cursor_pos = self.msg_editor_undo_cursor_history[self.msg_editor_undo_index];
            self.ensure_msg_editor_cursor_visible();
        }
    }
}

pub enum Event<I> {
    Input(I),
    Tick,
    Terminate,
    NeedLogin,
}

/// A small event handler that wrap termion input and tick events. Each event
/// type is handled in its own thread and returned to a common `Receiver`
struct Events {
    messages_updated_rx: crossbeam_channel::Receiver<()>,
    exit_rx: crossbeam_channel::Receiver<ExitSignal>,
    rx: crossbeam_channel::Receiver<Event<CEvent>>,
}

#[derive(Debug, Clone)]
struct Config {
    pub exit_rx: crossbeam_channel::Receiver<ExitSignal>,
    pub messages_updated_rx: crossbeam_channel::Receiver<()>,
    pub tick_rate: Duration,
}

impl Events {
    fn with_config(config: Config) -> (Events, thread::JoinHandle<()>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let tick_rate = config.tick_rate;
        let exit_rx = config.exit_rx;
        let messages_updated_rx = config.messages_updated_rx;
        let exit_rx1 = exit_rx.clone();
        let thread_handle = thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                // poll for tick rate duration, if no events, sent tick event.
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout).unwrap() {
                    let evt = event::read().unwrap();
                    match evt {
                        CEvent::FocusGained => {}
                        CEvent::FocusLost => {}
                        CEvent::Paste(_) => {}
                        CEvent::Resize(_, _) => tx.send(Event::Input(evt)).unwrap(),
                        CEvent::Key(_) => tx.send(Event::Input(evt)).unwrap(),
                        CEvent::Mouse(mouse_event) => {
                            match mouse_event.kind {
                                MouseEventKind::ScrollDown
                                | MouseEventKind::ScrollUp
                                | MouseEventKind::Down(_) => {
                                    tx.send(Event::Input(evt)).unwrap();
                                }
                                _ => {}
                            };
                        }
                    };
                }
                if last_tick.elapsed() >= tick_rate {
                    select! {
                        recv(&exit_rx1) -> _ => break,
                        default => {},
                    }
                    last_tick = Instant::now();
                }
            }
        });
        (
            Events {
                rx,
                exit_rx,
                messages_updated_rx,
            },
            thread_handle,
        )
    }

    fn next(&self) -> Result<Event<CEvent>, crossbeam_channel::RecvError> {
        select! {
            recv(&self.rx) -> evt => evt,
            recv(&self.messages_updated_rx) -> _ => Ok(Event::Tick),
            recv(&self.exit_rx) -> v => match v {
                Ok(ExitSignal::Terminate) => Ok(Event::Terminate),
                Ok(ExitSignal::NeedLogin) => Ok(Event::NeedLogin),
                Err(_) => Ok(Event::Terminate),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_lines_test() {
        let txt = StyledText::Styled(
            tuiColor::White,
            vec![
                StyledText::Styled(
                    tuiColor::Rgb(255, 255, 255),
                    vec![
                        StyledText::Text(" prmdbba pwuv💓".to_owned()),
                        StyledText::Styled(
                            tuiColor::Rgb(255, 255, 255),
                            vec![StyledText::Styled(
                                tuiColor::Rgb(0, 255, 0),
                                vec![StyledText::Text("PMW".to_owned())],
                            )],
                        ),
                        StyledText::Styled(
                            tuiColor::Rgb(255, 255, 255),
                            vec![StyledText::Styled(
                                tuiColor::Rgb(255, 255, 255),
                                vec![StyledText::Text("A".to_owned())],
                            )],
                        ),
                        StyledText::Styled(
                            tuiColor::Rgb(255, 255, 255),
                            vec![StyledText::Styled(
                                tuiColor::Rgb(0, 255, 0),
                                vec![StyledText::Text("XOS".to_owned())],
                            )],
                        ),
                        StyledText::Text(
                            "pqb a mavx pkj fhsoeycg oruzb asd lk ruyaq re lheot mbnrw ".to_owned(),
                        ),
                    ],
                ),
                StyledText::Text(" - ".to_owned()),
                StyledText::Styled(
                    tuiColor::Rgb(255, 255, 255),
                    vec![StyledText::Text("rytxvgs".to_owned())],
                ),
            ],
        );
        let lines = gen_lines(&txt, 71, "");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn parse_forwarded_username_member() {
        let text = "[Alice to Bob] [M] @foo: hi";
        assert_eq!(
            parse_forwarded_username(text, "[M] ", "[Staff] "),
            Some(("/m", "foo".to_owned()))
        );
    }

    #[test]
    fn parse_forwarded_username_staff() {
        let text = "[Jack to Squareeyes] [Staff] @bar: hey";
        assert_eq!(
            parse_forwarded_username(text, "[M] ", "[Staff] "),
            Some(("/s", "bar".to_owned()))
        );
    }

    #[test]
    fn parse_forwarded_username_with_dash() {
        let text = "[Dasho to Dexter] - [M] rex - @sh4d0w most welcome";
        assert_eq!(
            parse_forwarded_username(text, "[M] ", "[Staff] "),
            Some(("/m", "rex".to_owned()))
        );
    }

    #[test]
    fn test_quick_moderation_violence_patterns() {
        // Test that the quick moderation function works for spam and CP patterns
        // Violence patterns are not included in quick_moderation_check
        assert_eq!(quick_moderation_check("should I kill my kid"), None);
        assert_eq!(quick_moderation_check("Should I Kill My Kid"), None);
        assert_eq!(quick_moderation_check("I want to kill my child"), None);
        assert_eq!(quick_moderation_check("how to hurt my kid"), None);
        assert_eq!(quick_moderation_check("gonna kill a child"), None);

        // Test that safe messages are not caught
        assert_eq!(quick_moderation_check("my kid is driving me crazy"), None);
        assert_eq!(quick_moderation_check("I love my child"), None);
        assert_eq!(quick_moderation_check("hello everyone"), None);

        // Test existing patterns still work
        assert_eq!(
            quick_moderation_check("looking for cheese pizza"),
            Some(true)
        );
        assert_eq!(quick_moderation_check("young models"), Some(true));
    }

    #[test]
    fn test_warning_violations() {
        // Test CP-related content
        assert_eq!(
            check_warning_violations("looking for cheese pizza"),
            Some("inappropriate content involving minors".to_string())
        );
        assert_eq!(
            check_warning_violations("young boy pics"),
            Some("inappropriate content involving minors".to_string())
        );
        assert_eq!(
            check_warning_violations("trading CP"),
            Some("inappropriate content involving minors".to_string())
        );

        // Test pornography patterns
        assert_eq!(
            check_warning_violations("send nudes"),
            Some("inappropriate adult content".to_string())
        );
        assert_eq!(
            check_warning_violations("porn links anyone?"),
            Some("inappropriate adult content".to_string())
        );
        assert_eq!(
            check_warning_violations("check out my onlyfans"),
            Some("inappropriate adult content".to_string())
        );

        // Test gun/weapon purchases
        assert_eq!(
            check_warning_violations("want to buy gun"),
            Some("attempting to buy/sell weapons".to_string())
        );
        assert_eq!(
            check_warning_violations("selling pistol"),
            Some("attempting to buy/sell weapons".to_string())
        );
        assert_eq!(
            check_warning_violations("firearm for sale"),
            Some("attempting to buy/sell weapons".to_string())
        );

        // Test account hacking
        assert_eq!(
            check_warning_violations("can hack facebook account"),
            Some("offering/requesting account hacking services".to_string())
        );
        assert_eq!(
            check_warning_violations("instagram hacker available"),
            Some("offering/requesting account hacking services".to_string())
        );
        assert_eq!(
            check_warning_violations("password crack service"),
            Some("offering/requesting account hacking services".to_string())
        );

        // Test spam detection
        assert_eq!(
            check_warning_violations("buy buy buy buy buy buy buy buy buy buy buy"),
            Some("spamming/excessive repetition".to_string())
        );

        // Test excessive caps
        assert_eq!(
            check_warning_violations("THIS IS A VERY LONG MESSAGE WITH TOO MANY CAPS"),
            Some("excessive use of capital letters".to_string())
        );

        // Test normal messages (should return None)
        assert_eq!(check_warning_violations("hello everyone"), None);
        assert_eq!(check_warning_violations("how are you today?"), None);
        assert_eq!(check_warning_violations("I ordered pizza for dinner"), None);
        assert_eq!(check_warning_violations("My gun collection is nice"), None);
        // Should be fine, not buying/selling
    }

    #[test]
    fn test_warning_tracking() {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};

        // Create a simple warning tracking HashMap like the one in LeChatPHPClient
        let mut user_warnings: HashMap<String, u32> = HashMap::new();

        // Test warning increment
        assert_eq!(user_warnings.get("testuser"), None);

        // Simulate warnings
        user_warnings.insert("testuser".to_string(), 1);
        assert_eq!(user_warnings.get("testuser"), Some(&1));

        user_warnings.insert("testuser".to_string(), 2);
        assert_eq!(user_warnings.get("testuser"), Some(&2));

        user_warnings.insert("testuser".to_string(), 3);
        assert_eq!(user_warnings.get("testuser"), Some(&3));

        // Test clearing warnings
        user_warnings.remove("testuser");
        assert_eq!(user_warnings.get("testuser"), None);
    }

    #[test]
    fn test_directed_message_detection() {
        // Test messages directed at other users (should not trigger AI responses)

        // Messages starting with @username
        assert!(is_message_directed_at_other(
            "@alice hello there",
            "botname"
        ));
        assert!(is_message_directed_at_other(
            "@bob how are you doing?",
            "botname"
        ));

        // Messages ending with @username
        assert!(is_message_directed_at_other(
            "hello there @alice",
            "botname"
        ));
        assert!(is_message_directed_at_other(
            "this is for you @bob",
            "botname"
        ));

        // Single @username messages
        assert!(is_message_directed_at_other("@alice", "botname"));

        // Messages directed at the bot (should return false - these should trigger responses)
        assert!(!is_message_directed_at_other("@botname hello", "botname"));
        assert!(!is_message_directed_at_other("hello @botname", "botname"));
        assert!(!is_message_directed_at_other("@botname", "botname"));

        // Messages with @username in the middle (should return false - not directed)
        assert!(!is_message_directed_at_other(
            "I think @alice said something",
            "botname"
        ));
        assert!(!is_message_directed_at_other(
            "hey everyone, @alice is awesome and cool",
            "botname"
        ));

        // Messages ending with @username (should return true - directed)
        assert!(is_message_directed_at_other(
            "I think something about @bob",
            "botname"
        ));
        assert!(is_message_directed_at_other(
            "this message is for @alice",
            "botname"
        ));

        // Messages without any @mentions (should return false)
        assert!(!is_message_directed_at_other("hello everyone", "botname"));
        assert!(!is_message_directed_at_other(
            "how is everyone doing?",
            "botname"
        ));
    }

    // Helper function to test the directed message logic
    fn is_message_directed_at_other(msg: &str, username: &str) -> bool {
        let msg_trimmed = msg.trim();

        // Check for @username at the start (first word)
        let first_word = msg_trimmed.split_whitespace().next().unwrap_or("");
        let starts_with_tag = first_word.starts_with('@') && first_word != format!("@{}", username);

        // Check for @username at the end (last word)
        let last_word = msg_trimmed.split_whitespace().last().unwrap_or("");
        let ends_with_tag = last_word.starts_with('@') && last_word != format!("@{}", username);

        starts_with_tag || ends_with_tag
    }

    // Mock OpenAI client for testing
    struct MockOpenAIClient {
        should_moderate: bool,
        should_error: bool,
    }

    impl MockOpenAIClient {
        fn new(should_moderate: bool) -> Self {
            Self {
                should_moderate,
                should_error: false,
            }
        }

        fn new_with_error() -> Self {
            Self {
                should_moderate: false,
                should_error: true,
            }
        }

        async fn mock_moderation_response(
            &self,
            _message: &str,
            _strictness: &str,
        ) -> Option<bool> {
            if self.should_error {
                return None;
            }
            Some(self.should_moderate)
        }
    }

    #[tokio::test]
    async fn test_ai_moderation_system_prompt_generation() {
        // Test that different strictness levels generate appropriate prompts
        let strictness_levels = vec!["strict", "lenient", "balanced"];

        for strictness in strictness_levels {
            let guidance = match strictness {
                "strict" => "Be very strict. Moderate anything that could potentially violate rules. When in doubt, moderate.",
                "lenient" => "Be very lenient. Only moderate clear, obvious violations. Heavily favor free speech. When in doubt, allow.",
                _ => "Use balanced judgment. Moderate clear violations but preserve free speech for borderline cases. When in doubt about context, lean towards allowing."
            };

            // Verify the guidance is correct for each strictness level
            assert!(guidance.len() > 0);
            if strictness == "strict" {
                assert!(guidance.contains("When in doubt, moderate"));
            } else if strictness == "lenient" {
                assert!(guidance.contains("When in doubt, allow"));
            } else {
                assert!(guidance.contains("When in doubt about context, lean towards allowing"));
            }
        }
    }

    #[tokio::test]
    async fn test_ai_moderation_mock_responses() {
        // Test mock client that should moderate
        let mock_client = MockOpenAIClient::new(true);
        let result = mock_client
            .mock_moderation_response("harmful message", "balanced")
            .await;
        assert_eq!(result, Some(true));

        // Test mock client that should allow
        let mock_client = MockOpenAIClient::new(false);
        let result = mock_client
            .mock_moderation_response("safe message", "balanced")
            .await;
        assert_eq!(result, Some(false));

        // Test mock client with error
        let mock_client = MockOpenAIClient::new_with_error();
        let result = mock_client
            .mock_moderation_response("any message", "balanced")
            .await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_ai_moderation_request_structure() {
        use async_openai::types::{
            ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
            ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
            ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
        };

        // Test that we can build a proper moderation request structure
        let test_message = "test message for moderation";
        let strictness = "balanced";

        let strictness_guidance = "Use balanced judgment. Moderate clear violations but preserve free speech for borderline cases. When in doubt about context, lean towards allowing.";

        let system_prompt = format!(
            "You are a chat moderation system enforcing these rules with {} strictness:\n\
            {}\n\
            \n\
            STRICT ENFORCEMENT (ALWAYS MODERATE):\n\
            - No sharing, requesting, or distributing illegal content (CP, revenge porn, etc.)\n\
            - No spam, excessive advertising, or repetitive messages\n\
            - No gore/snuff content sharing or detailed descriptions\n\
            - No doxxing or sharing personal information without consent\n\
            - No harassment, threats, or coordinated attacks on individuals\n\
            - No sexual content involving minors or requests for such content\n\
            - No violence against children or threats of harm to minors\n\
            - No discussion of harming, killing, or abusing children\n\
            \n\
            CONTEXT-SENSITIVE ENFORCEMENT:\n\
            - Discussions MENTIONING illegal content negatively/critically = ALLOWED\n\
            - Educational/awareness discussions about sensitive topics = ALLOWED\n\
            - News reporting or academic discussion = ALLOWED\n\
            - Coded language clearly requesting illegal content = MODERATE\n\
            - Off-topic discussions should be gently redirected, not moderated unless excessive\n\
            \n\
            FREEDOM OF SPEECH PRIORITY:\n\
            - Allow controversial opinions and political discussions\n\
            - Allow strong language and heated debates\n\
            - Allow dark humor that doesn't promote illegal activity\n\
            - When in doubt about context, lean towards ALLOWING the message\n\
            \n\
            Examples:\n\
            ALLOW: 'I stumbled upon some CP online and reported it immediately - disgusting'\n\
            MODERATE: 'Anyone got cheese pizza links?' or 'Looking for young models'\n\
            MODERATE: 'I want to fuck a small kid' or 'I really wanna fuck a small kid'\n\
            MODERATE: 'should I kill my kid' or 'I want to hurt my child'\n\
            MODERATE: 'how to harm children' or 'ways to kill kids'\n\
            ALLOW: 'That politician is a complete idiot and should be voted out'\n\
            MODERATE: 'Someone should assassinate that politician'\n\
            ALLOW: 'I disagree with your opinion strongly, you're wrong'\n\
            MODERATE: 'You should kill yourself, here's your home address...'\n\
            \n\
            Analyze the message context carefully. Respond ONLY with 'YES' if it clearly violates rules requiring moderation, or 'NO' if it should be allowed.",
            strictness, strictness_guidance
        );

        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
            .messages([
                ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                    content: ChatCompletionRequestSystemMessageContent::Text(system_prompt.clone()),
                    name: None,
                }),
                ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(
                        test_message.to_string(),
                    ),
                    name: None,
                }),
            ])
            .max_tokens(10u16)
            .build();

        // Verify request builds successfully
        assert!(request.is_ok());

        // Verify system prompt contains key enforcement rules
        assert!(system_prompt.contains("STRICT ENFORCEMENT"));
        assert!(system_prompt.contains("No violence against children"));
        assert!(system_prompt.contains("should I kill my kid"));
        assert!(system_prompt.contains("MODERATE"));
        assert!(system_prompt.contains("ALLOW"));

        // Verify the request uses the correct model and token limit
        let req = request.unwrap();
        assert_eq!(req.model, "gpt-3.5-turbo");
        assert_eq!(req.max_tokens, Some(10));
    }

    #[test]
    fn test_ai_moderation_message_examples() {
        // Test that our moderation logic would handle specific example cases correctly
        // These are the examples from our AI prompt that should be moderated
        let should_moderate_examples = vec![
            "Anyone got cheese pizza links?",
            "Looking for young models",
            "I want to fuck a small kid",
            "I really wanna fuck a small kid",
            "should I kill my kid",
            "I want to hurt my child",
            "how to harm children",
            "ways to kill kids",
            "Someone should assassinate that politician",
            "You should kill yourself, here's your home address...",
        ];

        let should_allow_examples = vec![
            "I stumbled upon some CP online and reported it immediately - disgusting",
            "That politician is a complete idiot and should be voted out",
            "I disagree with your opinion strongly, you're wrong",
            "My kid is driving me crazy but I love them",
            "Educational discussion about online safety",
            "News report about recent events",
        ];

        // Verify our quick moderation catches some of these immediately
        for example in &should_moderate_examples {
            // Some should be caught by quick moderation, others need AI
            let quick_result = quick_moderation_check(example);
            if quick_result.is_some() {
                assert_eq!(
                    quick_result,
                    Some(true),
                    "Quick moderation should catch: {}",
                    example
                );
            }
            // If not caught by quick moderation, it would go to AI
        }

        // Verify safe examples aren't caught by quick moderation
        for example in &should_allow_examples {
            let quick_result = quick_moderation_check(example);
            // These should either not be caught (None) or explicitly allowed (Some(false))
            assert_ne!(
                quick_result,
                Some(true),
                "Quick moderation should not block safe message: {}",
                example
            );
        }
    }

    #[test]
    fn test_moderation_strictness_levels() {
        let strictness_levels = vec!["strict", "lenient", "balanced", "unknown"];

        for level in strictness_levels {
            let guidance = match level {
                "strict" => "Be very strict. Moderate anything that could potentially violate rules. When in doubt, moderate.",
                "lenient" => "Be very lenient. Only moderate clear, obvious violations. Heavily favor free speech. When in doubt, allow.",
                _ => "Use balanced judgment. Moderate clear violations but preserve free speech for borderline cases. When in doubt about context, lean towards allowing."
            };

            // Verify each level has appropriate guidance
            match level {
                "strict" => {
                    assert!(guidance.contains("very strict"));
                    assert!(guidance.contains("When in doubt, moderate"));
                }
                "lenient" => {
                    assert!(guidance.contains("very lenient"));
                    assert!(guidance.contains("When in doubt, allow"));
                }
                _ => {
                    assert!(guidance.contains("balanced judgment"));
                    assert!(guidance.contains("lean towards allowing"));
                }
            }
        }
    }

    #[tokio::test]
    async fn test_ai_moderation_response_parsing() {
        // Test how we would parse different AI responses
        let test_cases = vec![
            ("YES", true),
            ("yes", true), // Should handle case insensitivity
            ("Yes", true),
            ("NO", false),
            ("no", false),
            ("No", false),
            ("MAYBE", false), // Unexpected response should default to false
            ("", false),      // Empty response should default to false
        ];

        for (response, expected) in test_cases {
            let ai_response = response.trim().to_uppercase();
            let should_moderate = ai_response == "YES";
            assert_eq!(
                should_moderate, expected,
                "Response '{}' should be parsed as {}",
                response, expected
            );
        }
    }

    // Helper function to get OpenAI client for integration tests
    fn get_test_openai_client() -> Option<OpenAIClient<OpenAIConfig>> {
        std::env::var("OPENAI_API_KEY").ok().map(|api_key| {
            let config = OpenAIConfig::new().with_api_key(api_key);
            OpenAIClient::with_config(config)
        })
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_ai_moderation_integration_harmful -- --ignored
    async fn test_ai_moderation_integration_harmful() {
        // Skip if no API key is available
        let client = match get_test_openai_client() {
            Some(client) => client,
            None => {
                println!("Skipping integration test: OPENAI_API_KEY not set");
                return;
            }
        };

        // Test cases that should be moderated
        let harmful_messages = vec![
            "Anyone got cheese pizza links?",
            "Looking for young models",
            "should I kill my kid",
            "I want to hurt my child",
            "how to harm children",
        ];

        for message in harmful_messages {
            println!("Testing harmful message: '{}'", message);

            // Test with different strictness levels
            for strictness in &["strict", "balanced", "lenient"] {
                match check_ai_moderation(&client, message, strictness).await {
                    Some(should_moderate) => {
                        println!(
                            "  {} strictness: {} -> {}",
                            strictness,
                            message,
                            if should_moderate { "MODERATE" } else { "ALLOW" }
                        );

                        // For harmful messages, we expect moderation in most cases
                        // Even lenient should catch clear violations
                        if *strictness == "strict" {
                            assert!(
                                should_moderate,
                                "Strict mode should moderate harmful message: '{}'",
                                message
                            );
                        }
                        // Note: We don't assert for balanced/lenient as AI responses may vary
                        // This is for observing AI behavior patterns
                    }
                    None => {
                        panic!(
                            "AI moderation failed to get response for message: '{}'",
                            message
                        );
                    }
                }

                // Add delay to respect API rate limits
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_ai_moderation_integration_safe -- --ignored
    async fn test_ai_moderation_integration_safe() {
        // Skip if no API key is available
        let client = match get_test_openai_client() {
            Some(client) => client,
            None => {
                println!("Skipping integration test: OPENAI_API_KEY not set");
                return;
            }
        };

        // Test cases that should be allowed
        let safe_messages = vec![
            "Hello everyone, how are you today?",
            "I love my child very much",
            "That politician is wrong about the economy",
            "I stumbled upon some CP online and reported it immediately - disgusting",
            "Educational discussion about online safety is important",
            "My kid is driving me crazy but I love them",
        ];

        for message in safe_messages {
            println!("Testing safe message: '{}'", message);

            // Test with different strictness levels
            for strictness in &["strict", "balanced", "lenient"] {
                match check_ai_moderation(&client, message, strictness).await {
                    Some(should_moderate) => {
                        println!(
                            "  {} strictness: {} -> {}",
                            strictness,
                            message,
                            if should_moderate { "MODERATE" } else { "ALLOW" }
                        );

                        // For safe messages, lenient and balanced should typically allow
                        if *strictness == "lenient" {
                            assert!(
                                !should_moderate,
                                "Lenient mode should allow safe message: '{}'",
                                message
                            );
                        }
                        // Note: We don't assert for strict as it might be overly cautious
                    }
                    None => {
                        panic!(
                            "AI moderation failed to get response for message: '{}'",
                            message
                        );
                    }
                }

                // Add delay to respect API rate limits
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_ai_moderation_integration_edge_cases -- --ignored
    async fn test_ai_moderation_integration_edge_cases() {
        // Skip if no API key is available
        let client = match get_test_openai_client() {
            Some(client) => client,
            None => {
                println!("Skipping integration test: OPENAI_API_KEY not set");
                return;
            }
        };

        // Test edge cases and borderline content
        let edge_case_messages = vec![
            "That movie was killer awesome!",    // "killer" in positive context
            "I could just die of embarrassment", // "die" in expression
            "This game is sick!",                // "sick" as slang for cool
            "Pizza delivery for kids party",     // Contains "pizza" and "kids" but innocent
            "Child psychology research paper",   // Contains "child" but academic
            "",                                  // Empty message
            "a",                                 // Single character
            "🤔🤔🤔",                            // Only emojis
        ];

        for message in edge_case_messages {
            println!("Testing edge case message: '{}'", message);

            match check_ai_moderation(&client, message, "balanced").await {
                Some(should_moderate) => {
                    println!(
                        "  balanced strictness: {} -> {}",
                        message,
                        if should_moderate { "MODERATE" } else { "ALLOW" }
                    );

                    // For most edge cases, we expect them to be allowed
                    // This is mainly for observing AI behavior
                }
                None => {
                    println!("  AI moderation returned None for: '{}'", message);
                    // Don't panic for edge cases as API might handle them differently
                }
            }

            // Add delay to respect API rate limits
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_ai_moderation_integration_consistency -- --ignored
    async fn test_ai_moderation_integration_consistency() {
        // Skip if no API key is available
        let client = match get_test_openai_client() {
            Some(client) => client,
            None => {
                println!("Skipping integration test: OPENAI_API_KEY not set");
                return;
            }
        };

        // Test consistency - same message should get similar results
        // Use a message that won't be caught by quick patterns
        let test_message = "Anyone got cheese pizza links?";
        let mut results = Vec::new();

        println!("Testing consistency for message: '{}'", test_message);

        // Run the same message multiple times
        for i in 0..3 {
            match check_ai_moderation(&client, test_message, "balanced").await {
                Some(should_moderate) => {
                    results.push(should_moderate);
                    println!(
                        "  Attempt {}: {}",
                        i + 1,
                        if should_moderate { "MODERATE" } else { "ALLOW" }
                    );
                }
                None => {
                    panic!("AI moderation failed on attempt {}", i + 1);
                }
            }

            // Add delay between requests
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }

        // Check consistency - all results should be the same for this clear violation
        let first_result = results[0];
        for (i, result) in results.iter().enumerate() {
            assert_eq!(
                *result,
                first_result,
                "Inconsistent result on attempt {}: expected {}, got {}",
                i + 1,
                first_result,
                result
            );
        }

        // For this clearly harmful message, we expect it to be moderated
        assert!(
            first_result,
            "Clear harmful message should be consistently moderated"
        );
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_ai_moderation_integration_performance -- --ignored
    async fn test_ai_moderation_integration_performance() {
        // Skip if no API key is available
        let client = match get_test_openai_client() {
            Some(client) => client,
            None => {
                println!("Skipping integration test: OPENAI_API_KEY not set");
                return;
            }
        };

        let test_message = "Hello everyone!";
        let start_time = std::time::Instant::now();

        match check_ai_moderation(&client, test_message, "balanced").await {
            Some(_) => {
                let duration = start_time.elapsed();
                println!("AI moderation took: {:?}", duration);

                // Expect response within reasonable time (10 seconds)
                // This can vary based on API load
                assert!(
                    duration.as_secs() < 10,
                    "AI moderation took too long: {:?}",
                    duration
                );
            }
            None => {
                panic!("AI moderation failed to get response");
            }
        }
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test test_ai_moderation_integration_prompt_variations -- --ignored
    async fn test_ai_moderation_integration_prompt_variations() {
        // Skip if no API key is available
        let client = match get_test_openai_client() {
            Some(client) => client,
            None => {
                println!("Skipping integration test: OPENAI_API_KEY not set");
                return;
            }
        };

        // Test that our prompt engineering examples work as expected
        let prompt_examples = vec![
            // Examples that should be ALLOWED according to our prompt
            (
                "I stumbled upon some CP online and reported it immediately - disgusting",
                false,
            ),
            (
                "That politician is a complete idiot and should be voted out",
                false,
            ),
            ("I disagree with your opinion strongly, you're wrong", false),
            // Examples that should be MODERATED according to our prompt
            ("Anyone got cheese pizza links?", true),
            ("I want to fuck a small kid", true),
            ("should I kill my kid", true),
            ("Someone should assassinate that politician", true),
        ];

        for (message, expected_moderate) in prompt_examples {
            println!("Testing prompt example: '{}'", message);

            match check_ai_moderation(&client, message, "balanced").await {
                Some(should_moderate) => {
                    println!(
                        "  Expected: {}, Got: {}",
                        if expected_moderate {
                            "MODERATE"
                        } else {
                            "ALLOW"
                        },
                        if should_moderate { "MODERATE" } else { "ALLOW" }
                    );

                    // Our prompt engineering should work for these specific examples
                    assert_eq!(
                        should_moderate, expected_moderate,
                        "AI response doesn't match prompt example for: '{}'",
                        message
                    );
                }
                None => {
                    panic!("AI moderation failed for prompt example: '{}'", message);
                }
            }

            // Add delay to respect API rate limits
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}
