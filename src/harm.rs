use regex::Regex;

/// The type of harmful content detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    RacialSlur,
    CsabTalk,
    CsabRequest,
}

impl Reason {
    pub fn description(&self) -> &'static str {
        match self {
            Reason::RacialSlur => "using a racial slur (sorry if this is false)",
            Reason::CsabTalk => "referencing child sexual abuse material (sorry if this is false)",
            Reason::CsabRequest => "requesting child sexual abuse material (sorry if this is false)",
        }
    }
}

/// Result of scoring a message.
pub struct ScoreResult {
    pub score: u32,
    pub reason: Option<Reason>,
}

/// Return a severity score between 0 and 100 based on harmful content and
/// provide a reason when content is detected.
pub fn score_message(message: &str) -> ScoreResult {
    let msg = message.to_lowercase();
    let collapsed: String = msg.chars().filter(|c| c.is_alphanumeric()).collect();
    let normalized: String = collapsed
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' => 'i',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '7' => 't',
            _ => c,
        })
        .collect();

    let mut score = 0u32;
    let mut reason = None;

    // Detect uses of racial slurs (N-word and common variants)
    let nword_re = Regex::new(r"nigg(?:er|a)").unwrap();
    if nword_re.is_match(&msg) || normalized.contains("nigger") {
        let directed_re = Regex::new(r"(?:you|u|@\S+).{0,20}?nigg(?:er|a)").unwrap();
        if directed_re.is_match(&msg) {
            score = score.max(70);
        } else {
            score = score.max(40);
        }
        reason.get_or_insert(Reason::RacialSlur);
    }

    // Detect CSAM related talk (various obfuscations)
    let csam_terms = ["csam", "childporn", "pedo", "chees pizza", "childsex", "childsexualabuse", "cp"];
    if csam_terms.iter().any(|t| msg.contains(t) || normalized.contains(t)) {
        let request_re = Regex::new(
            r"\b(send|share|looking|where|has|download|anyone|link|give|provide)\b",
        )
        .unwrap();
        if request_re.is_match(&msg) {
            score = score.max(90);
            reason = Some(Reason::CsabRequest);
        } else {
            score = score.max(50);
            reason.get_or_insert(Reason::CsabTalk);
        }
    }

    if score > 100 {
        score = 100;
    }

    ScoreResult { score, reason }
}

/// Determine which action should be taken based on the score.
pub fn action_from_score(score: u32) -> Option<Action> {
    match score {
        0..=39 => None,
        40..=92 => Some(Action::Warn),
        93..=99 => Some(Action::Kick),
        _ => Some(Action::Ban),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Warn,
    Kick,
    Ban,
}
