#[derive(Debug, Clone, PartialEq)]
pub enum WarningType {
    LargeBandTransition {
        transition_width: f64,
        frequency_range: f64,
    },
}

impl WarningType {
    pub fn message(&self) -> String {
        match self {
            WarningType::LargeBandTransition {
                transition_width,
                frequency_range,
            } => {
                format!(
                    "Band transition too wide ({:.2} THz) compared to frequency range ({:.2} THz). \
                    Suggestion: reduce transition width.",
                    transition_width, frequency_range
                )
            }
        }
    }
}

/// Check if transition width is too large compared to frequency range
pub fn check_transition_width(
    start_freq: f64,
    end_freq: f64,
    win_width: f64,
) -> Option<WarningType> {
    let frequency_range = end_freq - start_freq;

    // Warning if transition width is more than 50% of the frequency range
    if win_width > frequency_range * 0.5 {
        Some(WarningType::LargeBandTransition {
            transition_width: win_width,
            frequency_range,
        })
    } else {
        None
    }
}
