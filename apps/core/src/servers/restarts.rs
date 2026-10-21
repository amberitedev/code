//! When a server restarts by itself: after a crash, on a schedule, or when it is empty and has
//! run long enough (smart restarts). The settings are per server, in the `restarts` table; a
//! server without a row has the defaults.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

/// A server's restart settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Restarts {
    /// Start the server again after a crash. On by default.
    pub crash: bool,
    /// A restart at a set time, with an in-game countdown.
    pub schedule: Option<Schedule>,
    /// Experimental: restart once nobody is online and the server has run this many hours.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "every", rename_all = "snake_case")]
pub enum Schedule {
    Daily {
        #[ts(type = "string")]
        at: TimeOfDay,
    },
    Weekly {
        on: Weekday,
        #[ts(type = "string")]
        at: TimeOfDay,
    },
}

/// `HH:MM`, 24-hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TS,
    clap::ValueEnum,
)]
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

/// A server's settings, the defaults without a row.
pub async fn get(db: &SqlitePool, server_id: &str) -> sqlx::Result<Restarts> {
    let row: Option<(bool, Option<String>, Option<u32>)> = sqlx::query_as(
        "SELECT crash, schedule, smart_after_hours FROM restarts WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await?;
    Ok(
        row.map_or_else(Restarts::default, |(crash, schedule, smart)| {
            Restarts {
                crash,
                schedule: schedule
                    .and_then(|json| serde_json::from_str(&json).ok()),
                smart_after_hours: smart,
            }
        }),
    )
}

pub async fn set(
    db: &SqlitePool,
    server_id: &str,
    restarts: &Restarts,
) -> sqlx::Result<()> {
    let schedule = restarts
        .schedule
        .map(|schedule| serde_json::to_string(&schedule).expect("serializes"));
    sqlx::query(
        "INSERT INTO restarts (server_id, crash, schedule, smart_after_hours) VALUES (?, ?, ?, ?) \
         ON CONFLICT (server_id) DO UPDATE SET crash = excluded.crash, \
         schedule = excluded.schedule, smart_after_hours = excluded.smart_after_hours",
    )
    .bind(server_id)
    .bind(restarts.crash)
    .bind(schedule)
    .bind(restarts.smart_after_hours)
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, servers::records};

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

    #[tokio::test]
    async fn settings_default_until_set() {
        let db = db::memory().await;
        let server = records::create(&db, "A").await.unwrap().unwrap();
        assert_eq!(get(&db, &server.id).await.unwrap(), Restarts::default());
        let changed = Restarts {
            crash: false,
            schedule: Some(Schedule::Daily {
                at: "04:00".parse().unwrap(),
            }),
            smart_after_hours: Some(12),
        };
        set(&db, &server.id, &changed).await.unwrap();
        assert_eq!(get(&db, &server.id).await.unwrap(), changed);
    }
}
