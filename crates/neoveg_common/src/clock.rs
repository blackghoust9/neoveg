//! The time of day, for the greeting of the welcome screen.
//!
//! The standard library knows only UTC, so the local hour comes from the `time` crate (the offset
//! of the operating system's time zone). When the system will not say, the hour is UTC.

/// The hour (`0..=23`) on the clock of this machine.
pub fn local_hour() -> u32 {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    u32::from(now.hour())
}

/// The part of the day an hour belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayPart {
    Morning,
    Afternoon,
    Night,
}

impl DayPart {
    /// Morning from 5 to 11, afternoon from 12 to 17, night otherwise.
    pub fn of_hour(hour: u32) -> Self {
        match hour {
            5..=11 => Self::Morning,
            12..=17 => Self::Afternoon,
            _ => Self::Night,
        }
    }

    /// The greeting with a `{}` where the name goes (the text is the key of the translations).
    pub fn greeting(self) -> &'static str {
        match self {
            Self::Morning => "Bom dia, {}",
            Self::Afternoon => "Boa tarde, {}",
            Self::Night => "Boa noite, {}",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_day_changes_part_at_five_noon_and_six() {
        assert_eq!(DayPart::of_hour(4), DayPart::Night);
        assert_eq!(DayPart::of_hour(5), DayPart::Morning);
        assert_eq!(DayPart::of_hour(11), DayPart::Morning);
        assert_eq!(DayPart::of_hour(12), DayPart::Afternoon);
        assert_eq!(DayPart::of_hour(17), DayPart::Afternoon);
        assert_eq!(DayPart::of_hour(18), DayPart::Night);
        assert_eq!(DayPart::of_hour(0), DayPart::Night);
        assert_eq!(DayPart::of_hour(23), DayPart::Night);
    }

    #[test]
    fn every_greeting_has_a_place_for_the_name() {
        for part in [DayPart::Morning, DayPart::Afternoon, DayPart::Night] {
            assert_eq!(part.greeting().matches("{}").count(), 1);
        }
    }

    #[test]
    fn the_local_hour_is_a_valid_hour() {
        assert!(local_hour() < 24);
    }
}
