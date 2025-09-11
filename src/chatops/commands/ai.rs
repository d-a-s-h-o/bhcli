use crate::ai_service::AIService;
use crate::chatops::{ChatCommand, ChatOpError, ChatOpResult, CommandContext};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// AI-powered message summarization
pub struct SummarizeCommand {
    ai_service: Arc<AIService>,
    runtime: Arc<Runtime>,
}

impl SummarizeCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for SummarizeCommand {
    fn name(&self) -> &'static str {
        "summarize"
    }
    fn description(&self) -> &'static str {
        "Summarize recent chat activity (AI)"
    }
    fn usage(&self) -> &'static str {
        "/summarize [count]"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["tldr", "summary"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if !self.ai_service.is_available() {
            return Ok(ChatOpResult::Error(
                "AI service not configured. Please check OPENAI_API_KEY environment variable."
                    .to_string(),
            ));
        }

        // Test if AI is actually functional (not just configured)
        let ai_service = Arc::clone(&self.ai_service);
        let is_functional = self
            .runtime
            .block_on(async move { ai_service.is_functional().await });

        if !is_functional {
            return Ok(ChatOpResult::Error(
                "AI service temporarily unavailable. This might be due to:\n\
                • API quota exceeded\n\
                • Billing issues\n\
                • Service outage\n\
                Please try again later or contact an admin."
                    .to_string(),
            ));
        }

        let count = if !args.is_empty() {
            args[0].parse::<usize>().unwrap_or(50).min(200)
        } else {
            50
        };

        let ai_service = Arc::clone(&self.ai_service);
        match self
            .runtime
            .block_on(async move { ai_service.summarize_chat(Some(count)).await })
        {
            Some(summary) => {
                let mut result = vec![
                    "🤖 **Chat Summary**".to_string(),
                    "".to_string(),
                    format!("**Overview:** {}", summary.summary),
                    "".to_string(),
                ];

                if !summary.key_points.is_empty() {
                    result.push("**Key Points:**".to_string());
                    for point in summary.key_points {
                        result.push(format!("• {}", point));
                    }
                    result.push("".to_string());
                }

                if !summary.participants.is_empty() {
                    result.push(format!(
                        "**Active Participants:** {}",
                        summary.participants.join(", ")
                    ));
                }

                if !summary.topics.is_empty() {
                    result.push(format!(
                        "**Topics Discussed:** {}",
                        summary.topics.join(", ")
                    ));
                }

                result.push(format!("**Overall Mood:** {}", summary.sentiment_overview));
                result.push(format!("*(Analyzed {} recent messages)*", count));

                Ok(ChatOpResult::Block(result))
            }
            None => Ok(ChatOpResult::Error(
                "Failed to generate summary. Please try again.".to_string(),
            )),
        }
    }
}

/// Language detection and translation command
pub struct TranslateCommand {
    ai_service: Arc<AIService>,
    runtime: Arc<Runtime>,
}

impl TranslateCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for TranslateCommand {
    fn name(&self) -> &'static str {
        "translate"
    }
    fn description(&self) -> &'static str {
        "Translate text to another language"
    }
    fn usage(&self) -> &'static str {
        "/translate <target_lang> <text>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["tr"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.len() < 2 {
            return Err(ChatOpError::MissingArguments(
                "Usage: /translate <language> <text>".to_string(),
            ));
        }

        let target_lang = &args[0];
        let text = args[1..].join(" ");

        // Try AI translation first if available
        if self.ai_service.is_available() {
            // Check if AI is functional
            let ai_service_check = Arc::clone(&self.ai_service);
            let is_functional = self
                .runtime
                .block_on(async move { ai_service_check.is_functional().await });

            if is_functional {
                let ai_service = Arc::clone(&self.ai_service);
                let target_lang_clone = target_lang.to_string();
                let text_clone = text.clone();

                match self.runtime.block_on(async move {
                    ai_service
                        .translate_text(&text_clone, &target_lang_clone)
                        .await
                }) {
                    Some(translated) => {
                        return Ok(ChatOpResult::Message(format!(
                            "🌐 **Translation to {}:**\n{}",
                            target_lang, translated
                        )));
                    }
                    None => {
                        // Fall through to basic translation
                    }
                }
            }
        }

        // Fallback to system translator
        match std::process::Command::new("trans")
            .arg("-b")
            .arg("-t")
            .arg(target_lang)
            .arg(&text)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let translated = String::from_utf8_lossy(&output.stdout);
                    Ok(ChatOpResult::Message(format!(
                        "🌐 **Translation to {}:**\n{}",
                        target_lang,
                        translated.trim()
                    )))
                } else {
                    let encoded_text = text.replace(" ", "%20");
                    Ok(ChatOpResult::Message(format!("🌐 Translation failed. Try: https://translate.google.com/?sl=auto&tl={}&text={}", target_lang, encoded_text)))
                }
            }
            Err(_) => {
                let encoded_text = text.replace(" ", "%20");
                Ok(ChatOpResult::Message(format!("🌐 System translator unavailable. Try: https://translate.google.com/?sl=auto&tl={}&text={}", target_lang, encoded_text)))
            }
        }
    }
}

/// Language detection command
pub struct DetectCommand {
    ai_service: Arc<AIService>,
    runtime: Arc<Runtime>,
}

impl DetectCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for DetectCommand {
    fn name(&self) -> &'static str {
        "detect"
    }
    fn description(&self) -> &'static str {
        "Detect the language of text"
    }
    fn usage(&self) -> &'static str {
        "/detect <text>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["lang"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify text to analyze".to_string(),
            ));
        }

        let text = args.join(" ");

        if self.ai_service.is_available() {
            // Check if AI is functional
            let ai_service_check = Arc::clone(&self.ai_service);
            let is_functional = self
                .runtime
                .block_on(async move { ai_service_check.is_functional().await });

            if !is_functional {
                return Ok(ChatOpResult::Error(
                    "AI service temporarily unavailable. Falling back to basic detection."
                        .to_string(),
                ));
            }

            let ai_service = Arc::clone(&self.ai_service);
            let text_clone = text.clone();
            match self
                .runtime
                .block_on(async move { ai_service.detect_language(&text_clone).await })
            {
                Some(detection) => {
                    let confidence_emoji = if detection.confidence > 0.8 {
                        "🎯"
                    } else if detection.confidence > 0.6 {
                        "🎲"
                    } else {
                        "❓"
                    };

                    Ok(ChatOpResult::Message(format!(
                        "{} **Language Detection:**\n**Language:** {} ({})\n**Confidence:** {:.0}%",
                        confidence_emoji,
                        detection.language,
                        detection.iso_code,
                        detection.confidence * 100.0
                    )))
                }
                None => {
                    // Fallback to simple detection
                    let detection = crate::ai_service::fallback_language_detection(&text);
                    Ok(ChatOpResult::Message(format!(
                        "🔍 **Language Detection (Basic):**\n**Language:** {} ({})\n**Confidence:** {:.0}%",
                        detection.language,
                        detection.iso_code,
                        detection.confidence * 100.0
                    )))
                }
            }
        } else {
            let detection = crate::ai_service::fallback_language_detection(&text);
            Ok(ChatOpResult::Message(format!(
                "🔍 **Language Detection (Basic):**\n**Language:** {} ({})\n**Confidence:** {:.0}%",
                detection.language,
                detection.iso_code,
                detection.confidence * 100.0
            )))
        }
    }
}

/// Sentiment analysis command
pub struct SentimentCommand {
    ai_service: Arc<AIService>,
    runtime: Arc<Runtime>,
}

impl SentimentCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for SentimentCommand {
    fn name(&self) -> &'static str {
        "sentiment"
    }
    fn description(&self) -> &'static str {
        "Analyze sentiment and emotions in text"
    }
    fn usage(&self) -> &'static str {
        "/sentiment <text>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["mood", "feel"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify text to analyze".to_string(),
            ));
        }

        let text = args.join(" ");

        if self.ai_service.is_available() {
            // Check if AI is functional
            let ai_service_check = Arc::clone(&self.ai_service);
            let is_functional = self
                .runtime
                .block_on(async move { ai_service_check.is_functional().await });

            if !is_functional {
                return Ok(ChatOpResult::Error(
                    "AI service temporarily unavailable. Cannot analyze sentiment.".to_string(),
                ));
            }

            let ai_service = Arc::clone(&self.ai_service);
            let text_clone = text.clone();
            match self
                .runtime
                .block_on(async move { ai_service.analyze_sentiment(&text_clone).await })
            {
                Some(analysis) => {
                    let sentiment_emoji = match analysis.sentiment.as_str() {
                        "positive" => "😊",
                        "negative" => "😔",
                        _ => "😐",
                    };

                    let mut result = vec![
                        format!("{} **Sentiment Analysis:**", sentiment_emoji),
                        format!(
                            "**Sentiment:** {} (Score: {:.2})",
                            analysis.sentiment, analysis.score
                        ),
                        format!("**Confidence:** {:.0}%", analysis.confidence * 100.0),
                    ];

                    if !analysis.emotions.is_empty() {
                        result.push(format!("**Emotions:** {}", analysis.emotions.join(", ")));
                    }

                    Ok(ChatOpResult::Block(result))
                }
                None => {
                    // Fallback to simple analysis
                    let analysis = crate::ai_service::fallback_sentiment_analysis(&text);
                    let sentiment_emoji = match analysis.sentiment.as_str() {
                        "positive" => "😊",
                        "negative" => "😔",
                        _ => "😐",
                    };

                    Ok(ChatOpResult::Message(format!(
                        "{} **Sentiment (Basic):** {} (Score: {:.2})",
                        sentiment_emoji, analysis.sentiment, analysis.score
                    )))
                }
            }
        } else {
            let analysis = crate::ai_service::fallback_sentiment_analysis(&text);
            let sentiment_emoji = match analysis.sentiment.as_str() {
                "positive" => "😊",
                "negative" => "😔",
                _ => "😐",
            };

            Ok(ChatOpResult::Message(format!(
                "{} **Sentiment (Basic):** {} (Score: {:.2})",
                sentiment_emoji, analysis.sentiment, analysis.score
            )))
        }
    }
}

/// Chat atmosphere command
pub struct AtmosphereCommand {
    ai_service: Arc<AIService>,
}

impl AtmosphereCommand {
    pub fn new(ai_service: Arc<AIService>) -> Self {
        Self { ai_service }
    }
}

impl ChatCommand for AtmosphereCommand {
    fn name(&self) -> &'static str {
        "atmosphere"
    }
    fn description(&self) -> &'static str {
        "Get current chat mood and activity"
    }
    fn usage(&self) -> &'static str {
        "/atmosphere"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["vibe", "mood"]
    }

    fn execute(
        &self,
        _args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let atmosphere = self.ai_service.get_chat_atmosphere();
        Ok(ChatOpResult::Message(format!(
            "**Current Chat Atmosphere:**\n{}",
            atmosphere
        )))
    }
}

/// Advanced moderation test command
pub struct ModCheckCommand {
    ai_service: Arc<AIService>,
    runtime: Arc<Runtime>,
}

impl ModCheckCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for ModCheckCommand {
    fn name(&self) -> &'static str {
        "modcheck"
    }
    fn description(&self) -> &'static str {
        "Test AI moderation on a message"
    }
    fn usage(&self) -> &'static str {
        "/modcheck <text>"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["checkmod"]
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if !self.ai_service.is_available() {
            return Ok(ChatOpResult::Error(
                "AI service not configured. Check OPENAI_API_KEY environment variable.".to_string(),
            ));
        }

        // Test if AI is actually functional (not just configured)
        let ai_service_check = Arc::clone(&self.ai_service);
        let is_functional = self
            .runtime
            .block_on(async move { ai_service_check.is_functional().await });

        if !is_functional {
            return Ok(ChatOpResult::Error(
                "AI moderation service temporarily unavailable. This might be due to:\n\
                • API quota exceeded\n\
                • Billing issues\n\
                • Service outage\n\
                Please try again later or contact an admin."
                    .to_string(),
            ));
        }

        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify text to check".to_string(),
            ));
        }

        let text = args.join(" ");
        let recent_msgs = self.ai_service.get_recent_messages(5);
        let context = recent_msgs
            .iter()
            .map(|m| format!("{}: {}", m.author, m.content))
            .collect::<Vec<_>>()
            .join(" | ");

        let ai_service = Arc::clone(&self.ai_service);
        match self
            .runtime
            .block_on(async move { ai_service.advanced_moderation(&text, &context).await })
        {
            Some(result) => {
                let action_emoji = match result.suggested_action.as_str() {
                    "ban" => "🔨",
                    "kick" => "👢",
                    "warn" => "⚠️",
                    _ => "✅",
                };

                let mut response = vec![
                    format!("{} **AI Moderation Check:**", action_emoji),
                    format!(
                        "**Should Moderate:** {}",
                        if result.should_moderate { "YES" } else { "NO" }
                    ),
                    format!("**Severity:** {}/10", result.severity),
                    format!("**Suggested Action:** {}", result.suggested_action),
                    format!("**Confidence:** {:.0}%", result.confidence * 100.0),
                ];

                if !result.reasons.is_empty() {
                    response.push("**Reasons:**".to_string());
                    for reason in result.reasons {
                        response.push(format!("• {}", reason));
                    }
                }

                Ok(ChatOpResult::Block(response))
            }
            None => Ok(ChatOpResult::Error(
                "Failed to analyze message. Please try again.".to_string(),
            )),
        }
    }
}

/// AI service status command
pub struct AIStatusCommand {
    ai_service: Arc<AIService>,
}

impl AIStatusCommand {
    pub fn new(ai_service: Arc<AIService>) -> Self {
        Self { ai_service }
    }
}

impl ChatCommand for AIStatusCommand {
    fn name(&self) -> &'static str {
        "aistatus"
    }
    fn description(&self) -> &'static str {
        "Check AI service status and statistics"
    }
    fn usage(&self) -> &'static str {
        "/aistatus"
    }
    fn aliases(&self) -> Vec<&'static str> {
        vec!["aiinfo"]
    }

    fn execute(
        &self,
        _args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        let stats = self.ai_service.get_stats();

        let mut result = vec!["🤖 **AI Service Status:**".to_string(), "".to_string()];

        for (key, value) in stats {
            let display_key = match key.as_str() {
                "available" => "Service Available",
                "message_history" => "Messages in History",
                "language_cache" => "Language Cache Size",
                "sentiment_cache" => "Sentiment Cache Size",
                "max_history" => "Max History Size",
                _ => &key,
            };
            result.push(format!("**{}:** {}", display_key, value));
        }

        result.push("".to_string());
        result.push("Available Commands: /summarize, /translate, /detect, /sentiment, /atmosphere, /modcheck".to_string());

        Ok(ChatOpResult::Block(result))
    }
}

/// Code fixing command with AI enhancement
pub struct FixCommand {
    ai_service: Arc<AIService>,
    #[allow(dead_code)]
    runtime: Arc<Runtime>,
}

impl FixCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for FixCommand {
    fn name(&self) -> &'static str {
        "fix"
    }
    fn description(&self) -> &'static str {
        "Attempt to fix code issues (AI)"
    }
    fn usage(&self) -> &'static str {
        "/fix <code>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify code to fix".to_string(),
            ));
        }

        let code = args.join(" ");

        // Enhanced basic analysis
        let mut suggestions = vec!["🔧 **Code Fix Suggestions:**".to_string()];

        // Language detection
        if code.contains("fn ") || code.contains("let ") || code.contains("mut ") {
            suggestions.push(
                "• **Rust detected**: Use `cargo check` and `clippy` for detailed analysis"
                    .to_string(),
            );
        } else if code.contains("def ") || code.contains("import ") {
            suggestions.push(
                "• **Python detected**: Check indentation and use `pylint` or `flake8`".to_string(),
            );
        } else if code.contains("function ") || code.contains("const ") || code.contains("=>") {
            suggestions.push(
                "• **JavaScript detected**: Use ESLint for comprehensive checking".to_string(),
            );
        }

        // Common issues
        if code.contains("unwrap()") {
            suggestions.push(
                "• Replace `unwrap()` with proper error handling using `?` or `match`".to_string(),
            );
        }

        if code.contains("panic!") {
            suggestions.push("• Consider returning `Result` instead of using `panic!`".to_string());
        }

        if !code.contains("//") && !code.contains("#") && !code.contains("/*") && code.len() > 50 {
            suggestions.push("• Add comments to explain complex logic".to_string());
        }

        if code.len() > 300 {
            suggestions
                .push("• Consider breaking this into smaller, more focused functions".to_string());
        }

        // AI enhancement note
        if self.ai_service.is_available() {
            suggestions.push("".to_string());
            suggestions
                .push("💡 *For detailed AI-powered code review, use `/review <code>`*".to_string());
        } else {
            suggestions.push("".to_string());
            suggestions.push(
                "💡 *Enable AI service (OPENAI_API_KEY) for enhanced code analysis*".to_string(),
            );
        }

        Ok(ChatOpResult::Block(suggestions))
    }
}

/// Enhanced code review command
pub struct ReviewCommand {
    ai_service: Arc<AIService>,
    runtime: Arc<Runtime>,
}

impl ReviewCommand {
    pub fn new(ai_service: Arc<AIService>, runtime: Arc<Runtime>) -> Self {
        Self {
            ai_service,
            runtime,
        }
    }
}

impl ChatCommand for ReviewCommand {
    fn name(&self) -> &'static str {
        "review"
    }
    fn description(&self) -> &'static str {
        "Get comprehensive code review (AI)"
    }
    fn usage(&self) -> &'static str {
        "/review <code>"
    }

    fn execute(
        &self,
        args: Vec<String>,
        _context: &CommandContext,
    ) -> Result<ChatOpResult, ChatOpError> {
        if args.is_empty() {
            return Err(ChatOpError::MissingArguments(
                "Please specify code to review".to_string(),
            ));
        }

        let code = args.join(" ");

        // Check if AI is functional
        let ai_service_check = Arc::clone(&self.ai_service);
        let is_functional = if self.ai_service.is_available() {
            self.runtime
                .block_on(async move { ai_service_check.is_functional().await })
        } else {
            false
        };

        if !is_functional {
            // Fallback to enhanced basic review
            let mut suggestions = vec!["📝 **Code Review (Basic Analysis):**".to_string()];

            if code.len() > 200 {
                suggestions.push(
                    "• Consider breaking this into smaller functions for better maintainability"
                        .to_string(),
                );
            }

            if code.contains("TODO") || code.contains("FIXME") || code.contains("XXX") {
                suggestions
                    .push("• Address TODO/FIXME comments before production deployment".to_string());
            }

            if !code.contains("//") && !code.contains("#") && !code.contains("/*") {
                suggestions.push(
                    "• Add descriptive comments explaining the logic and purpose".to_string(),
                );
            }

            if code.contains("panic!") || code.contains("unwrap()") {
                suggestions
                    .push("• Implement proper error handling instead of panicking".to_string());
            }

            if code.split('\n').count() > 20 {
                suggestions.push(
                    "• Function appears long - consider extracting helper functions".to_string(),
                );
            }

            // Variable naming check
            if code.chars().filter(|c| c.is_uppercase()).count() as f32 / code.len() as f32 > 0.3 {
                suggestions
                    .push("• Check variable naming conventions for your language".to_string());
            }

            suggestions.push("".to_string());
            suggestions
                .push("💡 *For AI-powered detailed review, set up OPENAI_API_KEY*".to_string());

            return Ok(ChatOpResult::Block(suggestions));
        }

        // AI-powered review
        let ai_service = Arc::clone(&self.ai_service);
        let prompt = format!(
            "Please review this code and provide specific, actionable feedback. Focus on:
- Code quality and best practices
- Potential bugs or issues
- Performance considerations
- Readability and maintainability
- Security concerns if applicable

Code to review:
```
{}
```

Please format your response as a markdown list of specific suggestions.",
            code
        );

        match self.runtime.block_on(async move {
            ai_service.translate_text(&prompt, "code review").await // Using translate as a general AI completion
        }) {
            Some(review) => {
                let result = vec!["🤖 **AI Code Review:**".to_string(), "".to_string(), review];
                Ok(ChatOpResult::Block(result))
            }
            None => Ok(ChatOpResult::Error(
                "AI review failed. Please try again or use basic review.".to_string(),
            )),
        }
    }
}
