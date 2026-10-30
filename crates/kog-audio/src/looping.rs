//! How long looping songs play: emulated formats (VGM, NSF, SPC, KSS, ...)
//! loop forever unless told when to stop. One process-wide choice, saved
//! with Kog's preferences and read by every decoder that loops.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const SETTING_FILE: &str = "loop-policy";

/// Songs without any length information play this long under the loop
/// counts (Once, Twice), as they always have.
const UNTIMED_LENGTH: Duration = Duration::from_secs(150);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopPolicy {
    /// Play to the loop point once, fading as the song starts over.
    Once,
    /// Play the loop twice, then fade.
    #[default]
    Twice,
    /// Keep looping for about this many minutes, then fade.
    Minutes(u32),
}

/// Every choice, in the order the settings show them.
pub const CHOICES: [LoopPolicy; 5] = [
    LoopPolicy::Once,
    LoopPolicy::Twice,
    LoopPolicy::Minutes(2),
    LoopPolicy::Minutes(3),
    LoopPolicy::Minutes(5),
];

impl LoopPolicy {
    pub fn setting_value(self) -> String {
        match self {
            Self::Once => "once".to_owned(),
            Self::Twice => "twice".to_owned(),
            Self::Minutes(minutes) => format!("{minutes}min"),
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim() {
            "once" => Some(Self::Once),
            "twice" => Some(Self::Twice),
            other => other
                .strip_suffix("min")
                .and_then(|minutes| minutes.parse::<u32>().ok())
                .filter(|minutes| (1..=60).contains(minutes))
                .map(Self::Minutes),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Once => "Play once".to_owned(),
            Self::Twice => "Loop once more".to_owned(),
            Self::Minutes(minutes) => format!("Loop for {minutes} minutes"),
        }
    }

    /// The next choice, for settings that cycle.
    pub fn next(self) -> Self {
        let at = CHOICES.iter().position(|choice| *choice == self).unwrap_or(1);
        CHOICES[(at + 1) % CHOICES.len()]
    }

    /// How many times to play a loop of `loop_length` after an intro of
    /// `intro`: once or twice, or as many as reach the time limit.
    pub fn loops(self, intro: Duration, loop_length: Duration) -> u32 {
        match self {
            Self::Once => 1,
            Self::Twice => 2,
            Self::Minutes(minutes) => {
                let limit = Duration::from_secs(u64::from(minutes) * 60);
                if loop_length.is_zero() {
                    return 1;
                }
                let remaining = limit.saturating_sub(intro).as_secs_f64();
                (remaining / loop_length.as_secs_f64()).round().clamp(1.0, 1000.0) as u32
            }
        }
    }

    /// A loop count for players that cannot report their loop's length
    /// before playing (Furnace, Organya): a time limit plays twice there.
    pub fn fixed_loops(self) -> u32 {
        match self {
            Self::Once => 1,
            _ => 2,
        }
    }

    /// The fade at the end of a looping song.
    pub fn fade(self) -> Duration {
        match self {
            Self::Once => Duration::from_secs(4),
            _ => Duration::from_secs(8),
        }
    }

    /// How long a song with no length information plays before its fade.
    pub fn untimed_length(self) -> Duration {
        match self {
            Self::Minutes(minutes) => Duration::from_secs(u64::from(minutes) * 60),
            _ => UNTIMED_LENGTH,
        }
    }

    fn encode(self) -> u32 {
        match self {
            Self::Once => 1,
            Self::Twice => 2,
            Self::Minutes(minutes) => 1000 + minutes,
        }
    }

    fn decode(value: u32) -> Self {
        match value {
            1 => Self::Once,
            1001..=1060 => Self::Minutes(value - 1000),
            _ => Self::Twice,
        }
    }
}

static CURRENT: AtomicU32 = AtomicU32::new(2);

/// The policy decoders follow in this process.
pub fn current() -> LoopPolicy {
    LoopPolicy::decode(CURRENT.load(Ordering::Relaxed))
}

/// Use `policy` for songs opened from now on.
pub fn set(policy: LoopPolicy) {
    CURRENT.store(policy.encode(), Ordering::Relaxed);
}

/// The saved choice, or the default.
pub fn saved() -> LoopPolicy {
    crate::settings::load_text(SETTING_FILE)
        .and_then(|value| LoopPolicy::from_setting(&value))
        .unwrap_or_default()
}

/// Apply the saved choice to this process.
pub fn load() -> LoopPolicy {
    let policy = saved();
    set(policy);
    policy
}

/// Save `policy` and apply it to this process.
pub fn save(policy: LoopPolicy) -> Result<(), String> {
    set(policy);
    crate::settings::save_text(SETTING_FILE, &policy.setting_value())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_loop_counts() {
        for choice in CHOICES {
            assert_eq!(LoopPolicy::from_setting(&choice.setting_value()), Some(choice));
            assert_eq!(LoopPolicy::decode(choice.encode()), choice);
        }
        let intro = Duration::from_secs(20);
        let body = Duration::from_secs(40);
        assert_eq!(LoopPolicy::Once.loops(intro, body), 1);
        assert_eq!(LoopPolicy::Twice.loops(intro, body), 2);
        assert_eq!(LoopPolicy::Minutes(2).loops(intro, body), 3);
        assert_eq!(LoopPolicy::Minutes(3).loops(intro, body), 4);
    }
}
