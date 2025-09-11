use crate::ai_service::AIService;
use crate::bot_system::{BotConfig, BotSystem, MessageType};
use crate::PostType;
use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use log::{error, info, warn};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Bot client that runs in the background
pub struct BotClient {
    bot_name: String,
    bot_system: Arc<BotSystem>,
    running: Arc<Mutex<bool>>,
    tx: Sender<PostType>,
    rx: Arc<Mutex<crossbeam_channel::Receiver<PostType>>>,
}

impl BotClient {
    /// Create a new bot client
    pub fn new(
        bot_name: String,
        _username: String,
        _password: String,
        _url: String,
        ai_service: Option<Arc<AIService>>,
        runtime: Option<Arc<Runtime>>,
        admin_users: Vec<String>,
    ) -> Result<Self> {
        // Create communication channels
        let (tx, rx) = crossbeam_channel::unbounded();

        // Create bot configuration
        let bot_config = BotConfig {
            bot_name: bot_name.clone(),
            admin_users,
            data_directory: std::env::current_dir()?.join("bot_data").join(&bot_name),
            ..BotConfig::default()
        };

        // Create bot system
        let bot_system = Arc::new(BotSystem::new(bot_config, tx.clone(), ai_service, runtime)?);

        Ok(Self {
            bot_name,
            bot_system,
            running: Arc::new(Mutex::new(false)),
            tx,
            rx: Arc::new(Mutex::new(rx)),
        })
    }

    /// Start the bot client
    pub fn start(&mut self) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                return Err(anyhow!("Bot client is already running"));
            }
            *running = true;
        }

        info!("Starting bot client: {}", self.bot_name);

        // Start bot system
        self.bot_system.start()?;

        // Since we can't easily create a separate LeChatPHPClient instance,
        // we'll simulate the bot by creating a background thread that processes
        // messages and responds to commands.
        self.start_message_processing_thread()?;

        Ok(())
    }

    /// Stop the bot client
    pub fn stop(&mut self) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            if !*running {
                return Ok(());
            }
            *running = false;
        }

        info!("Stopping bot client: {}", self.bot_name);

        // Stop bot system
        self.bot_system.stop()?;

        Ok(())
    }

    /// Start message processing thread
    fn start_message_processing_thread(&self) -> Result<()> {
        let bot_system = Arc::clone(&self.bot_system);
        let running = Arc::clone(&self.running);
        let bot_name = self.bot_name.clone();
        let tx = self.tx.clone();

        thread::spawn(move || {
            info!("Bot {} message processing thread started", bot_name);

            // Send initial status message (non-blocking)
            let startup_msg = format!("🤖 {} is now online and ready to help!", bot_name);
            if let Err(e) = tx.try_send(PostType::Post(startup_msg, Some("0".to_string()))) {
                warn!(
                    "Could not send bot startup message (channel may be disconnected): {}",
                    e
                );
            }

            while *running.lock().unwrap() {
                // In a real implementation, this would:
                // 1. Monitor main chat messages for @bot mentions
                // 2. Process the messages through the bot system
                // 3. Send appropriate responses

                // For now, we'll just maintain the bot system state
                thread::sleep(Duration::from_secs(30));

                // Send periodic status if needed (for debugging)
                if bot_system.is_running() {
                    // Bot is active and ready to respond to commands
                    // Commands will be processed when main client detects @bot mentions
                }
            }

            // Send shutdown message (non-blocking)
            let shutdown_msg = format!("🤖 {} is going offline. Data has been saved.", bot_name);
            if let Err(e) = tx.try_send(PostType::Post(shutdown_msg, Some("0".to_string()))) {
                warn!(
                    "Could not send bot shutdown message (channel may be disconnected): {}",
                    e
                );
            }

            info!("Bot {} message processing thread stopped", bot_name);
        });

        Ok(())
    }

    /// Process a message for the bot system
    #[allow(dead_code)]
    pub fn process_message(
        &self,
        username: &str,
        content: &str,
        message_id: Option<usize>,
        is_private: bool,
    ) -> Result<()> {
        // Determine message type
        let message_type = if is_private {
            MessageType::PrivateMessage {
                to: self.bot_name.clone(),
            }
        } else {
            MessageType::Normal
        };

        // Process through bot system
        self.bot_system.process_message(
            username,
            content,
            message_type,
            message_id.map(|id| id as u64),
            None,  // No channel context for individual bot calls
            false, // Assume non-member for individual calls
        )?;

        Ok(())
    }

    /// Check if bot is running
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// Get bot name
    pub fn get_bot_name(&self) -> &str {
        &self.bot_name
    }
}

impl Drop for BotClient {
    fn drop(&mut self) {
        if self.is_running() {
            if let Err(e) = self.stop() {
                error!("Failed to stop bot client during drop: {}", e);
            }
        }
    }
}

/// Bot manager to handle multiple bots
pub struct BotManager {
    bots: Vec<BotClient>,
    ai_service: Option<Arc<AIService>>,
    runtime: Option<Arc<Runtime>>,
}

impl BotManager {
    pub fn new(ai_service: Option<Arc<AIService>>, runtime: Option<Arc<Runtime>>) -> Self {
        Self {
            bots: Vec::new(),
            ai_service,
            runtime,
        }
    }

    pub fn add_bot(
        &mut self,
        bot_name: String,
        username: String,
        password: String,
        url: String,
        admin_users: Vec<String>,
    ) -> Result<()> {
        let bot = BotClient::new(
            bot_name,
            username,
            password,
            url,
            self.ai_service.clone(),
            self.runtime.clone(),
            admin_users,
        )?;

        self.bots.push(bot);
        Ok(())
    }

    /// Get all bot receivers for message forwarding
    pub fn get_all_bot_receivers(
        &self,
    ) -> Vec<(String, Arc<Mutex<crossbeam_channel::Receiver<PostType>>>)> {
        self.bots
            .iter()
            .map(|bot| (bot.get_bot_name().to_string(), Arc::clone(&bot.rx)))
            .collect()
    }

    pub fn start_bot(&mut self, bot_name: &str) -> Result<()> {
        if let Some(bot) = self.bots.iter_mut().find(|b| b.get_bot_name() == bot_name) {
            bot.start()?;
            info!("Started bot: {}", bot_name);
        } else {
            return Err(anyhow!("Bot not found: {}", bot_name));
        }
        Ok(())
    }

    /// Stop all bots
    pub fn stop_all(&mut self) -> Result<()> {
        for bot in &mut self.bots {
            if bot.is_running() {
                bot.stop()?;
            }
        }
        Ok(())
    }

    /// Process message for all bots
    pub fn process_message_for_all_bots(
        &self,
        username: &str,
        content: &str,
        message_type: MessageType,
        message_id: Option<u64>,
        channel_context: Option<&str>,
        is_member: bool,
    ) -> Result<()> {
        for bot in &self.bots {
            if bot.is_running() {
                if let Err(e) = bot.bot_system.process_message(
                    username,
                    content,
                    message_type.clone(),
                    message_id,
                    channel_context,
                    is_member,
                ) {
                    warn!(
                        "Failed to process message for bot {}: {}",
                        bot.get_bot_name(),
                        e
                    );
                }
            }
        }
        Ok(())
    }
}
