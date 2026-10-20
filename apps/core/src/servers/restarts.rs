//! When a server restarts by itself: after a crash, on a schedule, or when it is empty and has
//! run long enough (smart restarts). The settings are per server, in the `restarts` table; a
//! server without a row has the defaults. Crash restarts are in `watch`; [`run`] does the other
//! two.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Datelike, Days, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{info, warn};
use ts_rs::TS;

use super::{Phase, Servers, insight::Insight};
use crate::access::now;

/// How many seconds before a scheduled restart players are warned in the game.
pub const WARNINGS: [i64; 3] = [300, 60, 10];
/// How often [`run`] looks at the schedules.
const TICK: Duration = Duration::from_secs(15);

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

impl From<Weekday> for chrono::Weekday {
    fn from(day: Weekday) -> Self {
        match day {
            Weekday::Monday => Self::Mon,
            Weekday::Tuesday => Self::Tue,
            Weekday::Wednesday => Self::Wed,
            Weekday::Thursday => Self::Thu,
            Weekday::Friday => Self::Fri,
            Weekday::Saturday => Self::Sat,
            Weekday::Sunday => Self::Sun,
        }
    }
}

/// The first time `schedule` names after `after`, in `after`'s time zone. A time the clock skips
/// that day, at a daylight saving change, moves to the next day it exists.
pub fn next<Tz: TimeZone>(
    schedule: Schedule,
    after: &DateTime<Tz>,
) -> DateTime<Tz> {
    let (day, at) = match schedule {
        Schedule::Daily { at } => (None, at),
        Schedule::Weekly { on, at } => (Some(chrono::Weekday::from(on)), at),
    };
    let time = NaiveTime::from_hms_opt(at.hour.into(), at.minute.into(), 0)
        .expect("a checked time of day");
    let zone = after.timezone();
    let today = after.date_naive();
    (0..=8)
        .filter_map(|offset| today.checked_add_days(Days::new(offset)))
        .filter(|date| day.is_none_or(|day| date.weekday() == day))
        .filter_map(|date| {
            zone.from_local_datetime(&date.and_time(time)).earliest()
        })
        .find(|candidate| candidate > after)
        .expect("a matching day comes within a week")
}

/// Whether a smart restart is due: the server ran `after_hours` and `list` says nobody is on.
pub fn smart_due(after_hours: u32, uptime_s: i64, insight: &Insight) -> bool {
    insight.players_checked
        && insight.players.is_empty()
        && uptime_s >= i64::from(after_hours) * 3600
}

/// The countdown warning for `left` seconds.
fn warning(left: i64) -> String {
    match left {
        60.. if left % 60 == 0 => {
            let minutes = left / 60;
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!("say The server restarts in {minutes} {unit}.")
        }
        _ => format!("say The server restarts in {left} seconds."),
    }
}

/// When the server's current run started, in unix seconds.
async fn run_started_at(db: &SqlitePool, server_id: &str) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT started_at FROM runs WHERE server_id = ? AND ended_at IS NULL          ORDER BY started_at DESC LIMIT 1",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Restart running servers on their schedules, with a countdown in the game, and empty ones
/// whose smart restart is due. Runs as long as Core does.
pub async fn run(servers: Arc<Servers>) {
    let busy = Arc::new(Mutex::new(HashSet::<String>::new()));
    let mut tick = tokio::time::interval(TICK);
    loop {
        tick.tick().await;
        for id in servers.running() {
            if busy.lock().expect("not poisoned").contains(&id) {
                continue;
            }
            let Ok(settings) = get(&servers.db, &id).await else {
                continue;
            };
            let at = settings.schedule.map(|schedule| {
                next(schedule, &chrono::Local::now()).timestamp()
            });
            let smart = match settings.smart_after_hours {
                Some(hours) => {
                    let uptime = run_started_at(&servers.db, &id)
                        .await
                        .map_or(0, |started| now() - started);
                    servers.insight(&id).is_some_and(|insight| {
                        smart_due(hours, uptime, &insight)
                    })
                }
                None => false,
            };
            let at = match at {
                Some(at) if at - now() <= WARNINGS[0] => at,
                _ if smart => now(),
                _ => continue,
            };
            busy.lock().expect("not poisoned").insert(id.clone());
            let (servers, busy) = (Arc::clone(&servers), Arc::clone(&busy));
            tokio::spawn(async move {
                countdown(&servers, &id, at).await;
                busy.lock().expect("not poisoned").remove(&id);
            });
        }
    }
}

/// Warn the players at each of [`WARNINGS`] still ahead of `at`, then restart the server. Gives
/// up when it stops running in the meantime.
async fn countdown(servers: &Servers, id: &str, at: i64) {
    for left in WARNINGS {
        let wait = at - left - now();
        if wait < 0 {
            continue;
        }
        tokio::time::sleep(Duration::from_secs(wait as u64)).await;
        if servers.phase(id) != Some(Phase::Running) {
            return;
        }
        let _ = servers.command(id, &warning(left)).await;
    }
    let wait = at - now();
    if wait > 0 {
        tokio::time::sleep(Duration::from_secs(wait as u64)).await;
    }
    if servers.phase(id) != Some(Phase::Running) {
        return;
    }
    info!(server = %id, "restarting on its schedule");
    if let Err(error) = servers.restart(id).await {
        warn!(server = %id, %error, "a scheduled restart failed");
    }
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

    fn utc(text: &str) -> DateTime<chrono::Utc> {
        text.parse().unwrap()
    }

    #[test]
    fn a_daily_restart_is_today_or_tomorrow() {
        let daily = Schedule::Daily {
            at: "04:00".parse().unwrap(),
        };
        assert_eq!(
            next(daily, &utc("2026-10-20T03:59:00Z")),
            utc("2026-10-20T04:00:00Z")
        );
        assert_eq!(
            next(daily, &utc("2026-10-20T04:00:00Z")),
            utc("2026-10-21T04:00:00Z"),
            "the restart that just ran is not due again"
        );
        assert_eq!(
            next(daily, &utc("2026-12-31T23:00:00Z")),
            utc("2027-01-01T04:00:00Z")
        );
    }

    #[test]
    fn a_weekly_restart_is_on_its_day() {
        // 2026-10-20 is a Tuesday.
        let sunday = Schedule::Weekly {
            on: Weekday::Sunday,
            at: "05:30".parse().unwrap(),
        };
        assert_eq!(
            next(sunday, &utc("2026-10-20T12:00:00Z")),
            utc("2026-10-25T05:30:00Z")
        );
        let tuesday = Schedule::Weekly {
            on: Weekday::Tuesday,
            at: "05:30".parse().unwrap(),
        };
        assert_eq!(
            next(tuesday, &utc("2026-10-20T05:00:00Z")),
            utc("2026-10-20T05:30:00Z")
        );
        assert_eq!(
            next(tuesday, &utc("2026-10-20T06:00:00Z")),
            utc("2026-10-27T05:30:00Z")
        );
        // In the machine's own zone, the wall-clock time holds.
        let zone = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
        let after = zone.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
        assert_eq!(
            next(sunday, &after),
            zone.with_ymd_and_hms(2026, 10, 25, 5, 30, 0).unwrap()
        );
    }

    #[test]
    fn players_are_warned_before_a_restart() {
        assert_eq!(warning(300), "say The server restarts in 5 minutes.");
        assert_eq!(warning(60), "say The server restarts in 1 minute.");
        assert_eq!(warning(10), "say The server restarts in 10 seconds.");
    }

    #[test]
    fn a_smart_restart_waits_for_uptime_and_an_empty_checked_server() {
        let mut insight = Insight {
            players_checked: true,
            ..Insight::default()
        };
        assert!(smart_due(6, 6 * 3600, &insight));
        assert!(!smart_due(6, 6 * 3600 - 1, &insight));
        insight.players.insert("Steve".into());
        assert!(!smart_due(6, 7 * 3600, &insight));
        let unchecked = Insight::default();
        assert!(
            !smart_due(6, 7 * 3600, &unchecked),
            "nobody known to be online is not nobody online"
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
