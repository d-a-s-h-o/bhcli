use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
    },
    Client as OpenAIClient,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDetection {
    pub language: String,
    pub confidence: f64,
    pub iso_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    pub sentiment: String, // "positive", "negative", "neutral"
    pub confidence: f64,
    pub score: f64,            // -1.0 to 1.0
    pub emotions: Vec<String>, // anger, joy, fear, sadness, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    pub summary: String,
    pub key_points: Vec<String>,
    pub participants: Vec<String>,
    pub topics: Vec<String>,
    pub sentiment_overview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationResult {
    pub should_moderate: bool,
    pub severity: u8, // 0-10
    pub reasons: Vec<String>,
    pub suggested_action: String, // "none", "warn", "kick", "ban"
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub author: String,
    pub content: String,
    pub is_pm: bool,
}

pub struct AIService {
    client: Option<OpenAIClient<OpenAIConfig>>,
    message_history: Arc<Mutex<Vec<ChatMessage>>>,
    language_cache: Arc<Mutex<HashMap<String, LanguageDetection>>>,
    sentiment_cache: Arc<Mutex<HashMap<String, SentimentAnalysis>>>,
    max_history: usize,
}

impl AIService {
    pub fn new() -> Self {
        let client = std::env::var("OPENAI_API_KEY").ok().map(|api_key| {
            let config = OpenAIConfig::new().with_api_key(api_key);
            OpenAIClient::with_config(config)
        });

        Self {
            client,
            message_history: Arc::new(Mutex::new(Vec::new())),
            language_cache: Arc::new(Mutex::new(HashMap::new())),
            sentiment_cache: Arc::new(Mutex::new(HashMap::new())),
            max_history: 1000,
        }
    }

    pub fn is_available(&self) -> bool {
        self.client.is_some()
    }

    /// Check if AI can actually be used (not just configured)
    /// This tests for credit exhaustion and API availability
    pub async fn is_functional(&self) -> bool {
        if !self.is_available() {
            return false;
        }

        self.test_api_connection().await.unwrap_or(false)
    }

    /// Test API connection with minimal request to check for credit/quota issues
    async fn test_api_connection(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(client) = &self.client {
            let request = CreateChatCompletionRequestArgs::default()
                .max_tokens(1u16)
                .model("gpt-3.5-turbo")
                .messages([ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: ChatCompletionRequestSystemMessageContent::Text(
                            "test".to_string(),
                        ),
                        name: None,
                    },
                )])
                .build()?;

            match timeout(Duration::from_secs(5), client.chat().create(request)).await {
                Ok(Ok(_)) => Ok(true),
                Ok(Err(e)) => {
                    // Check for specific credit exhaustion errors
                    let error_msg = e.to_string().to_lowercase();
                    if error_msg.contains("quota")
                        || error_msg.contains("credit")
                        || error_msg.contains("billing")
                        || error_msg.contains("insufficient")
                        || error_msg.contains("exceeded")
                    {
                        log::warn!("AI service unavailable due to credit/quota issues: {}", e);
                        Ok(false)
                    } else {
                        log::error!("AI service test failed: {}", e);
                        Ok(false)
                    }
                }
                Err(_) => {
                    log::warn!("AI service test timed out");
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }

    pub fn add_message(&self, message: ChatMessage) {
        let mut history = self.message_history.lock().unwrap();
        history.push(message);

        // Keep only the last max_history messages
        if history.len() > self.max_history {
            let excess = history.len() - self.max_history;
            history.drain(0..excess);
        }
    }

    pub fn get_recent_messages(&self, count: usize) -> Vec<ChatMessage> {
        let history = self.message_history.lock().unwrap();
        let start = if history.len() > count {
            history.len() - count
        } else {
            0
        };
        history[start..].to_vec()
    }

    pub async fn detect_language(&self, text: &str) -> Option<LanguageDetection> {
        // Check cache first
        {
            let cache = self.language_cache.lock().unwrap();
            if let Some(detection) = cache.get(text) {
                return Some(detection.clone());
            }
        }

        let client = self.client.as_ref()?;

        let prompt = format!(
            "Detect the language of the following text and return only a JSON response in this exact format:
{{
    \"language\": \"language_name\",
    \"confidence\": 0.95,
    \"iso_code\": \"ISO_639-1_code\"
}}

Text to analyze: \"{}\"

Important: Return ONLY the JSON, no other text or explanation.",
            text.trim()
        );

        let result = timeout(Duration::from_secs(10), async {
            let request = CreateChatCompletionRequestArgs::default()
                .model("gpt-3.5-turbo")
                .messages([
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(
                                "You are a language detection assistant. Always respond with valid JSON only.".to_string()
                            ),
                            name: None,
                        }
                    ),
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(prompt),
                            name: None,
                        }
                    ),
                ])
                .max_tokens(100u16)
                .temperature(0.1)
                .build()?;

            client.chat().create(request).await
        }).await;

        match result {
            Ok(Ok(response)) => {
                if let Some(choice) = response.choices.first() {
                    if let Some(content) = &choice.message.content {
                        if let Ok(detection) =
                            serde_json::from_str::<LanguageDetection>(content.trim())
                        {
                            // Cache the result
                            {
                                let mut cache = self.language_cache.lock().unwrap();
                                cache.insert(text.to_string(), detection.clone());

                                // Limit cache size
                                if cache.len() > 100 {
                                    let keys: Vec<String> =
                                        cache.keys().take(20).cloned().collect();
                                    for key in keys {
                                        cache.remove(&key);
                                    }
                                }
                            }
                            return Some(detection);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub async fn analyze_sentiment(&self, text: &str) -> Option<SentimentAnalysis> {
        // Check cache first
        {
            let cache = self.sentiment_cache.lock().unwrap();
            if let Some(analysis) = cache.get(text) {
                return Some(analysis.clone());
            }
        }

        let client = self.client.as_ref()?;

        let prompt = format!(
            "Analyze the sentiment and emotions of the following text and return only a JSON response in this exact format:
{{
    \"sentiment\": \"positive|negative|neutral\",
    \"confidence\": 0.85,
    \"score\": 0.3,
    \"emotions\": [\"joy\", \"excitement\"]
}}

Text to analyze: \"{}\"

Score should be between -1.0 (very negative) and 1.0 (very positive).
Emotions can include: joy, anger, fear, sadness, surprise, disgust, trust, anticipation.
Return ONLY the JSON, no other text.",
            text.trim()
        );

        let result = timeout(Duration::from_secs(10), async {
            let request = CreateChatCompletionRequestArgs::default()
                .model("gpt-3.5-turbo")
                .messages([
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(
                                "You are a sentiment analysis assistant. Always respond with valid JSON only.".to_string()
                            ),
                            name: None,
                        }
                    ),
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(prompt),
                            name: None,
                        }
                    ),
                ])
                .max_tokens(150u16)
                .temperature(0.1)
                .build()?;

            client.chat().create(request).await
        }).await;

        match result {
            Ok(Ok(response)) => {
                if let Some(choice) = response.choices.first() {
                    if let Some(content) = &choice.message.content {
                        if let Ok(analysis) =
                            serde_json::from_str::<SentimentAnalysis>(content.trim())
                        {
                            // Cache the result
                            {
                                let mut cache = self.sentiment_cache.lock().unwrap();
                                cache.insert(text.to_string(), analysis.clone());

                                // Limit cache size
                                if cache.len() > 100 {
                                    let keys: Vec<String> =
                                        cache.keys().take(20).cloned().collect();
                                    for key in keys {
                                        cache.remove(&key);
                                    }
                                }
                            }
                            return Some(analysis);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub async fn summarize_chat(&self, message_count: Option<usize>) -> Option<MessageSummary> {
        let client = self.client.as_ref()?;

        let count = message_count.unwrap_or(50);
        let messages = self.get_recent_messages(count);

        if messages.is_empty() {
            return None;
        }

        let chat_text = messages
            .iter()
            .map(|msg| {
                if msg.is_pm {
                    format!("[PM] {}: {}", msg.author, msg.content)
                } else {
                    format!("{}: {}", msg.author, msg.content)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Analyze and summarize the following chat conversation. Return only a JSON response in this exact format:
{{
    \"summary\": \"Brief summary of the conversation\",
    \"key_points\": [\"Point 1\", \"Point 2\"],
    \"participants\": [\"user1\", \"user2\"],
    \"topics\": [\"topic1\", \"topic2\"],
    \"sentiment_overview\": \"Overall mood description\"
}}

Chat messages:
{}

Return ONLY the JSON, no other text.",
            chat_text
        );

        let result = timeout(Duration::from_secs(15), async {
            let request = CreateChatCompletionRequestArgs::default()
                .model("gpt-3.5-turbo")
                .messages([
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(
                                "You are a chat summarization assistant. Always respond with valid JSON only.".to_string()
                            ),
                            name: None,
                        }
                    ),
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(prompt),
                            name: None,
                        }
                    ),
                ])
                .max_tokens(400u16)
                .temperature(0.3)
                .build()?;

            client.chat().create(request).await
        }).await;

        match result {
            Ok(Ok(response)) => {
                if let Some(choice) = response.choices.first() {
                    if let Some(content) = &choice.message.content {
                        if let Ok(summary) = serde_json::from_str::<MessageSummary>(content.trim())
                        {
                            return Some(summary);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub async fn advanced_moderation(&self, text: &str, context: &str) -> Option<ModerationResult> {
        let client = self.client.as_ref()?;

        let prompt = format!(
            "Analyze the following message for harmful content, considering the chat context. Return only a JSON response in this exact format:
{{
    \"should_moderate\": false,
    \"severity\": 3,
    \"reasons\": [\"reason1\", \"reason2\"],
    \"suggested_action\": \"warn\",
    \"confidence\": 0.85
}}

Message to analyze: \"{}\"
Chat context: \"{}\"

Severity scale: 0 (harmless) to 10 (extremely harmful)
Suggested actions: \"none\", \"warn\", \"kick\", \"ban\"
Consider: harassment, hate speech, spam, threats, inappropriate content, but also context and intent.
Return ONLY the JSON, no other text.",
            text.trim(),
            context.trim()
        );

        let result = timeout(Duration::from_secs(12), async {
            let request = CreateChatCompletionRequestArgs::default()
                .model("gpt-3.5-turbo")
                .messages([
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(
                                "You are a content moderation assistant for an anonymous chat. Be balanced - not too strict but protect users from genuine harm. Always respond with valid JSON only.".to_string()
                            ),
                            name: None,
                        }
                    ),
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(prompt),
                            name: None,
                        }
                    ),
                ])
                .max_tokens(200u16)
                .temperature(0.2)
                .build()?;

            client.chat().create(request).await
        }).await;

        match result {
            Ok(Ok(response)) => {
                if let Some(choice) = response.choices.first() {
                    if let Some(content) = &choice.message.content {
                        if let Ok(moderation) =
                            serde_json::from_str::<ModerationResult>(content.trim())
                        {
                            return Some(moderation);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub async fn translate_text(&self, text: &str, target_language: &str) -> Option<String> {
        let client = self.client.as_ref()?;

        let prompt = format!(
            "Translate the following text to {} and return ONLY the translated text, no explanations or quotes:

Text to translate: \"{}\"",
            target_language, text.trim()
        );

        let result = timeout(Duration::from_secs(10), async {
            let request = CreateChatCompletionRequestArgs::default()
                .model("gpt-3.5-turbo")
                .messages([
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessage {
                            content: ChatCompletionRequestSystemMessageContent::Text(
                                "You are a translation assistant. Always return only the translated text, nothing else.".to_string()
                            ),
                            name: None,
                        }
                    ),
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: ChatCompletionRequestUserMessageContent::Text(prompt),
                            name: None,
                        }
                    ),
                ])
                .max_tokens(300u16)
                .temperature(0.1)
                .build()?;

            client.chat().create(request).await
        }).await;

        match result {
            Ok(Ok(response)) => {
                if let Some(choice) = response.choices.first() {
                    choice.message.content.clone()
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn get_chat_atmosphere(&self) -> String {
        let messages = self.get_recent_messages(20);
        if messages.is_empty() {
            return "😐 Quiet".to_string();
        }

        let total_msgs = messages.len();
        let unique_users: std::collections::HashSet<_> =
            messages.iter().map(|m| &m.author).collect();
        let user_count = unique_users.len();

        // Simple heuristic for activity level
        let activity = if total_msgs > 15 {
            "Very Active"
        } else if total_msgs > 8 {
            "Active"
        } else if total_msgs > 3 {
            "Moderate"
        } else {
            "Quiet"
        };

        let diversity = if user_count > 5 {
            "Diverse"
        } else if user_count > 2 {
            "Social"
        } else {
            "Focused"
        };

        format!(
            "🌊 {} & {} ({} msgs, {} users)",
            activity, diversity, total_msgs, user_count
        )
    }

    pub fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        let history = self.message_history.lock().unwrap();
        let lang_cache = self.language_cache.lock().unwrap();
        let sentiment_cache = self.sentiment_cache.lock().unwrap();

        stats.insert("available".to_string(), self.is_available().to_string());
        stats.insert("message_history".to_string(), history.len().to_string());
        stats.insert("language_cache".to_string(), lang_cache.len().to_string());
        stats.insert(
            "sentiment_cache".to_string(),
            sentiment_cache.len().to_string(),
        );
        stats.insert("max_history".to_string(), self.max_history.to_string());

        stats
    }
}

// Helper function for fallback language detection using simple heuristics
pub fn fallback_language_detection(text: &str) -> LanguageDetection {
    let text_lower = text.to_lowercase();

    // Simple heuristic based on common words
    let english_indicators = ["the", "and", "is", "are", "you", "that", "with", "for"];
    let spanish_indicators = ["el", "la", "es", "con", "por", "que", "una", "para"];
    let french_indicators = ["le", "la", "et", "est", "avec", "pour", "une", "dans"];
    let german_indicators = ["der", "die", "und", "ist", "mit", "für", "eine", "das"];

    let english_count = english_indicators
        .iter()
        .filter(|&&word| text_lower.contains(word))
        .count();
    let spanish_count = spanish_indicators
        .iter()
        .filter(|&&word| text_lower.contains(word))
        .count();
    let french_count = french_indicators
        .iter()
        .filter(|&&word| text_lower.contains(word))
        .count();
    let german_count = german_indicators
        .iter()
        .filter(|&&word| text_lower.contains(word))
        .count();

    let max_count = *[english_count, spanish_count, french_count, german_count]
        .iter()
        .max()
        .unwrap_or(&0);

    if max_count == 0 {
        return LanguageDetection {
            language: "Unknown".to_string(),
            confidence: 0.1,
            iso_code: "??".to_string(),
        };
    }

    let confidence = (max_count as f64 / text.split_whitespace().count() as f64).min(0.9);

    if english_count == max_count {
        LanguageDetection {
            language: "English".to_string(),
            confidence,
            iso_code: "en".to_string(),
        }
    } else if spanish_count == max_count {
        LanguageDetection {
            language: "Spanish".to_string(),
            confidence,
            iso_code: "es".to_string(),
        }
    } else if french_count == max_count {
        LanguageDetection {
            language: "French".to_string(),
            confidence,
            iso_code: "fr".to_string(),
        }
    } else if german_count == max_count {
        LanguageDetection {
            language: "German".to_string(),
            confidence,
            iso_code: "de".to_string(),
        }
    } else {
        LanguageDetection {
            language: "Unknown".to_string(),
            confidence: 0.1,
            iso_code: "??".to_string(),
        }
    }
}

// Helper function for basic sentiment analysis without AI
pub fn fallback_sentiment_analysis(text: &str) -> SentimentAnalysis {
    let text_lower = text.to_lowercase();

    let positive_words = [
        "good",
        "great",
        "awesome",
        "amazing",
        "happy",
        "love",
        "excellent",
        "perfect",
        "wonderful",
    ];
    let negative_words = [
        "bad",
        "terrible",
        "awful",
        "hate",
        "sad",
        "angry",
        "horrible",
        "disgusting",
        "annoying",
    ];

    let positive_count = positive_words
        .iter()
        .filter(|&&word| text_lower.contains(word))
        .count() as f64;
    let negative_count = negative_words
        .iter()
        .filter(|&&word| text_lower.contains(word))
        .count() as f64;

    let total_words = text.split_whitespace().count() as f64;
    let score = (positive_count - negative_count) / total_words.max(1.0);

    let (sentiment, emotions) = if score > 0.1 {
        ("positive", vec!["joy".to_string()])
    } else if score < -0.1 {
        ("negative", vec!["anger".to_string()])
    } else {
        ("neutral", vec![])
    };

    SentimentAnalysis {
        sentiment: sentiment.to_string(),
        confidence: 0.6,
        score: score.clamp(-1.0, 1.0),
        emotions,
    }
}
