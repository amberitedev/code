//! What a Core adds to its servers: memory, JVM flags, restarts, Java, the console, and insight.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Performance {
    /// The most memory Java may use. Java starts with less and grows to it.
    pub memory_mb: u32,
    /// The JVM flags the server starts with.
    pub flags: Vec<String>,
    /// `flags` are the server's own rather than the Core's tuned ones.
    pub custom_flags: bool,
    pub restarts: Restarts,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Memory {
    pub memory_mb: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct JvmFlags {
    /// The server's own flags, each starting with `-`. Null goes back to the tuned ones.
    pub flags: Option<Vec<String>>,
}

/// A server's restart settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Restarts {
    /// Start the server again after a crash. On by default.
    pub crash: bool,
    /// A restart at a set time, with an in-game countdown.
    pub schedule: Option<Schedule>,
    /// Experimental: restart once nobody is online and the server has run this many hours,
    /// after a minute's warning in the game.
    pub smart_after_hours: Option<u32>,
}

impl Default for Restarts {
    fn default() -> Self {
        Self {
            crash: true,
            schedule: None,
            smart_after_hours: None,
        }
    }
}

/// A scheduled restart, in the Core machine's local time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "every", rename_all = "snake_case")]
pub enum Schedule {
    Daily {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        at: TimeOfDay,
    },
    Weekly {
        on: Weekday,
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        at: TimeOfDay,
    },
}

/// `HH:MM`, 24-hour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
}

impl TryFrom<String> for TimeOfDay {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl std::str::FromStr for TimeOfDay {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let invalid = || format!("{text} is not a time like 04:30");
        let (hour, minute) = text.split_once(':').ok_or_else(invalid)?;
        let hour: u8 = hour.parse().map_err(|_| invalid())?;
        let minute: u8 = minute.parse().map_err(|_| invalid())?;
        if hour > 23 || minute > 59 {
            return Err(invalid());
        }
        Ok(Self { hour, minute })
    }
}

impl From<TimeOfDay> for String {
    fn from(time: TimeOfDay) -> Self {
        format!("{:02}:{:02}", time.hour, time.minute)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConsoleCommand {
    /// One line, as typed into the server console, such as `say hello`.
    pub command: String,
}

/// A custom Java binary on the Core's machine, or null for the Java the Core manages. An advanced
/// override; it runs as `copal-servers`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct JavaPath {
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerInsight {
    /// Who is online, from the Core's health check and join and leave lines. Empty while the
    /// server does not run.
    pub players: Vec<String>,
    /// The health check answered this run, so `players` is checked, not only what join and
    /// leave lines said.
    pub players_checked: bool,
    /// The server stopped answering the Core's health check, so it is probably frozen. Offer a
    /// restart.
    pub frozen: bool,
    /// Unix seconds of the server's last answer to the health check.
    pub answered_at: Option<i64>,
    /// The Core stopped starting the server again after it crashed 3 times in 10 minutes.
    pub crash_restart_stopped: bool,
    /// The last crash, with the console lines before it for editors and up.
    pub last_crash: Option<Crash>,
    /// Whether players can join from the internet. Null while the server does not run or has
    /// not been checked yet.
    pub reachability: Option<super::ReachabilityReport>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Crash {
    /// Unix seconds.
    pub at: i64,
    pub exit_code: Option<i32>,
    /// The console lines before the crash. Null for viewers: they can hold chat, addresses,
    /// and anything a mod printed, which the console route keeps to editors.
    pub last_lines: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{Schedule, TimeOfDay, Weekday};

    #[test]
    fn times_of_day_are_checked() {
        let time: TimeOfDay = "04:30".parse().unwrap();
        assert_eq!(String::from(time), "04:30");
        for bad in ["4", "24:00", "12:60", "ab:cd", ""] {
            assert!(bad.parse::<TimeOfDay>().is_err(), "{bad}");
        }
        let schedule: Schedule = serde_json::from_str(
            r#"{"every":"weekly","on":"sunday","at":"05:00"}"#,
        )
        .unwrap();
        assert_eq!(
            schedule,
            Schedule::Weekly {
                on: Weekday::Sunday,
                at: TimeOfDay { hour: 5, minute: 0 }
            }
        );
    }
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        Performance::decl(&cfg),
        Memory::decl(&cfg),
        JvmFlags::decl(&cfg),
        Restarts::decl(&cfg),
        Schedule::decl(&cfg),
        Weekday::decl(&cfg),
        ConsoleCommand::decl(&cfg),
        JavaPath::decl(&cfg),
        ServerInsight::decl(&cfg),
        Crash::decl(&cfg),
    ]
}
