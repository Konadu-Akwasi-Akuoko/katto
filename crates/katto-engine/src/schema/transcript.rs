//! ElevenLabs Scribe v2 transcript shape (`transcript.json`). Decimal seconds:
//! this is a model-boundary wire format, never engine cut math.

use serde::{Deserialize, Serialize};

/// A parsed Scribe v2 transcription response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Transcript {
    /// Source audio duration in seconds as reported by Scribe.
    pub audio_duration_secs: Option<f64>,
    /// Detected language code (e.g. `"en"`).
    pub language_code: String,
    /// Scribe's confidence in the language detection.
    pub language_probability: f64,
    /// The full transcription text.
    pub text: String,
    /// Word-level tokens: words, spacing, and audio events.
    pub words: Vec<WordEntry>,
}

/// One transcript token, discriminated by Scribe's `type` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WordEntry {
    /// A spoken word with its timing and confidence.
    Word {
        /// The word text.
        text: String,
        /// Start time in seconds.
        start: f64,
        /// End time in seconds.
        end: f64,
        /// Log probability of the recognition (absent in some responses).
        logprob: Option<f64>,
        /// Diarized speaker id (e.g. `"speaker_0"`).
        speaker_id: Option<String>,
    },
    /// Inter-word spacing.
    Spacing {
        /// The spacing text (usually `" "`).
        text: String,
        /// Start time in seconds.
        start: f64,
        /// End time in seconds.
        end: f64,
    },
    /// A non-speech audio event such as `[breath]` or `[laughter]`.
    AudioEvent {
        /// The bracketed event label.
        text: String,
        /// Start time in seconds.
        start: f64,
        /// End time in seconds.
        end: f64,
    },
}

impl WordEntry {
    /// Start time in seconds, whatever the token type.
    pub fn start(&self) -> f64 {
        match self {
            Self::Word { start, .. }
            | Self::Spacing { start, .. }
            | Self::AudioEvent { start, .. } => *start,
        }
    }

    /// End time in seconds, whatever the token type.
    pub fn end(&self) -> f64 {
        match self {
            Self::Word { end, .. } | Self::Spacing { end, .. } | Self::AudioEvent { end, .. } => {
                *end
            }
        }
    }

    /// Token text, whatever the token type.
    pub fn text(&self) -> &str {
        match self {
            Self::Word { text, .. }
            | Self::Spacing { text, .. }
            | Self::AudioEvent { text, .. } => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn parses_scribe_v2_fixture() {
        let t: Transcript = serde_json::from_str(&fixture("transcript.valid.json")).unwrap();
        assert!(t.words.iter().any(|w| matches!(w, WordEntry::Word { .. })));
        assert!(t.audio_duration_secs.is_some());
    }

    #[test]
    fn word_entry_accessors_cover_all_variants() {
        let spacing: WordEntry =
            serde_json::from_str(r#"{"text":" ","type":"spacing","start":0.34,"end":0.47}"#)
                .unwrap();
        assert_eq!(spacing.start(), 0.34);
        assert_eq!(spacing.end(), 0.47);
        assert_eq!(spacing.text(), " ");
    }

    #[test]
    fn round_trips_preserving_tag() {
        let t: Transcript = serde_json::from_str(&fixture("transcript.valid.json")).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<Transcript>(&json).unwrap(), t);
    }
}
