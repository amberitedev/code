//! When a server restarts by itself: after a crash, on a schedule, or when it is empty and has
//! run long enough (smart restarts). The settings are per server, in the `restarts` table; a
//! server without a row has the defaults. Crash restarts are in `watch`; [`run`] does the other
//! two, each with an in-game countdown that belongs to one run of the server under the settings
//! it started with, and stops as soon as either changes.

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
/// The same for a smart restart. Who is online comes from the console, which can be wrong, so
/// a smart restart warns too, and stops when anyone joins.
const SMART_WARNINGS: [i64; 2] = [60, 10];
/// How often [`run`] looks at the schedules.
const TICK: Duration = Duration::from_secs(15);
/// How often a countdown checks that it still holds.
const STEP: i64 = 1;

/// A server's restart settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
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
/// at a daylight saving change does not happen that day, so the next day it does happen counts:
/// the next day for a daily restart, the next week for a weekly one. `None` when nothing comes
/// within three weeks, which no real time zone does.
pub fn next<Tz: TimeZone>(
    schedule: Schedule,
    after: &DateTime<Tz>,
) -> Option<DateTime<Tz>> {
    let (day, at) = match schedule {
        Schedule::Daily { at } => (None, at),
        Schedule::Weekly { on, at } => (Some(chrono::Weekday::from(on)), at),
    };
    let time = NaiveTime::from_hms_opt(at.hour.into(), at.minute.into(), 0)
        .expect("a checked time of day");
    let zone = after.timezone();
    let today = after.date_naive();
    (0..=21)
        .filter_map(|offset| today.checked_add_days(Days::new(offset)))
        .filter(|date| day.is_none_or(|day| date.weekday() == day))
        .filter_map(|date| {
            zone.from_local_datetime(&date.and_time(time)).earliest()
        })
        .find(|candidate| candidate > after)
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
        "SELECT started_at FROM runs WHERE server_id = ? AND ended_at IS NULL \
         ORDER BY started_at DESC LIMIT 1",
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
            let Some(bound) = Bound::now(&servers, &id).await else {
                continue;
            };
            let at = bound
                .settings
                .schedule
                .and_then(|schedule| next(schedule, &chrono::Local::now()))
                .map(|at| at.timestamp());
            let smart = match bound.settings.smart_after_hours {
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
            let (at, why) = match at {
                Some(at) if at - now() <= WARNINGS[0] => (at, Why::Schedule),
                _ if smart => (now() + SMART_WARNINGS[0], Why::Smart),
                _ => continue,
            };
            busy.lock().expect("not poisoned").insert(id.clone());
            let (servers, busy) = (Arc::clone(&servers), Arc::clone(&busy));
            tokio::spawn(async move {
                countdown(&servers, &id, at, why, &bound).await;
                busy.lock().expect("not poisoned").remove(&id);
            });
        }
    }
}

/// Why a countdown runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Why {
    Schedule,
    Smart,
}

/// What a countdown belongs to: one run of the server, under the settings it started with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bound {
    run: u64,
    settings: Restarts,
}

impl Bound {
    /// The server's run and settings now, `None` when it does not run.
    async fn now(servers: &Servers, id: &str) -> Option<Self> {
        Some(Self {
            run: servers.run_of(id)?,
            settings: get(&servers.db, id).await.ok()?,
        })
    }
}

/// Whether a countdown still holds: the run it is for still runs, under the same settings, and
/// for a smart restart, nobody is online.
async fn holds(servers: &Servers, id: &str, why: Why, bound: &Bound) -> bool {
    servers.phase(id) == Some(Phase::Running)
        && Bound::now(servers, id).await.as_ref() == Some(bound)
        && (why == Why::Schedule
            || servers
                .insight(id)
                .is_some_and(|insight| insight.players.is_empty()))
}

/// Wait until `until`, unix seconds, while the countdown holds. Whether it still does.
async fn wait_until(
    servers: &Servers,
    id: &str,
    why: Why,
    bound: &Bound,
    until: i64,
) -> bool {
    loop {
        if !holds(servers, id, why, bound).await {
            return false;
        }
        let left = until - now();
        if left <= 0 {
            return true;
        }
        tokio::time::sleep(Duration::from_secs(left.min(STEP) as u64)).await;
    }
}

/// Warn the players at each warning still ahead of `at`, then restart the server. Stops,
/// without a restart, as soon as the countdown no longer holds.
async fn countdown(
    servers: &Servers,
    id: &str,
    at: i64,
    why: Why,
    bound: &Bound,
) {
    let warnings: &[i64] = match why {
        Why::Schedule => &WARNINGS,
        Why::Smart => &SMART_WARNINGS,
    };
    for &left in warnings {
        if at - left < now() {
            continue;
        }
        if !wait_until(servers, id, why, bound, at - left).await {
            return;
        }
        let _ = servers.command(id, &warning(left)).await;
    }
    if !wait_until(servers, id, why, bound, at).await {
        return;
    }
    info!(server = %id, ?why, "restarting");
    if let Err(error) = servers.restart(id).await {
        warn!(server = %id, %error, ?why, "a restart failed");
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
            next(daily, &utc("2026-10-20T03:59:00Z")).unwrap(),
            utc("2026-10-20T04:00:00Z")
        );
        assert_eq!(
            next(daily, &utc("2026-10-20T04:00:00Z")).unwrap(),
            utc("2026-10-21T04:00:00Z"),
            "the restart that just ran is not due again"
        );
        assert_eq!(
            next(daily, &utc("2026-12-31T23:00:00Z")).unwrap(),
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
            next(sunday, &utc("2026-10-20T12:00:00Z")).unwrap(),
            utc("2026-10-25T05:30:00Z")
        );
        let tuesday = Schedule::Weekly {
            on: Weekday::Tuesday,
            at: "05:30".parse().unwrap(),
        };
        assert_eq!(
            next(tuesday, &utc("2026-10-20T05:00:00Z")).unwrap(),
            utc("2026-10-20T05:30:00Z")
        );
        assert_eq!(
            next(tuesday, &utc("2026-10-20T06:00:00Z")).unwrap(),
            utc("2026-10-27T05:30:00Z")
        );
        // In the machine's own zone, the wall-clock time holds.
        let zone = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
        let after = zone.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
        assert_eq!(
            next(sunday, &after).unwrap(),
            zone.with_ymd_and_hms(2026, 10, 25, 5, 30, 0).unwrap()
        );
    }

    /// Central Europe in spring 2026: at 02:00 on Sunday 29 March the clock goes to 03:00, so
    /// 02:00 to 03:00 does not happen that day.
    #[derive(Debug, Clone, Copy)]
    struct Spring;

    impl Spring {
        fn change() -> chrono::NaiveDateTime {
            "2026-03-29T02:00:00".parse().unwrap()
        }
        fn winter() -> chrono::FixedOffset {
            chrono::FixedOffset::east_opt(3600).unwrap()
        }
        fn summer() -> chrono::FixedOffset {
            chrono::FixedOffset::east_opt(2 * 3600).unwrap()
        }
    }

    impl TimeZone for Spring {
        type Offset = chrono::FixedOffset;

        fn from_offset(_: &Self::Offset) -> Self {
            Self
        }

        fn offset_from_local_date(
            &self,
            _: &chrono::NaiveDate,
        ) -> chrono::MappedLocalTime<Self::Offset> {
            unimplemented!("next only asks for times")
        }

        fn offset_from_local_datetime(
            &self,
            local: &chrono::NaiveDateTime,
        ) -> chrono::MappedLocalTime<Self::Offset> {
            if *local < Self::change() {
                chrono::MappedLocalTime::Single(Self::winter())
            } else if *local < Self::change() + chrono::TimeDelta::hours(1) {
                chrono::MappedLocalTime::None
            } else {
                chrono::MappedLocalTime::Single(Self::summer())
            }
        }

        fn offset_from_utc_date(&self, _: &chrono::NaiveDate) -> Self::Offset {
            unimplemented!("next only asks for times")
        }

        fn offset_from_utc_datetime(
            &self,
            utc: &chrono::NaiveDateTime,
        ) -> Self::Offset {
            if *utc < Self::change() - chrono::TimeDelta::hours(1) {
                Self::winter()
            } else {
                Self::summer()
            }
        }
    }

    #[test]
    fn a_time_the_clock_skips_moves_to_its_next_day() {
        let at = |text: &str| {
            Spring
                .from_local_datetime(&text.parse().unwrap())
                .single()
                .unwrap()
        };
        // On Friday, the only Sunday within the week is the one without 02:30.
        let sunday = Schedule::Weekly {
            on: Weekday::Sunday,
            at: "02:30".parse().unwrap(),
        };
        assert_eq!(
            next(sunday, &at("2026-03-27T12:00:00")),
            Some(at("2026-04-05T02:30:00"))
        );
        let daily = Schedule::Daily {
            at: "02:30".parse().unwrap(),
        };
        assert_eq!(
            next(daily, &at("2026-03-28T12:00:00")),
            Some(at("2026-03-30T02:30:00"))
        );
        // A time outside the gap that day is untouched.
        let after_gap = Schedule::Weekly {
            on: Weekday::Sunday,
            at: "04:00".parse().unwrap(),
        };
        assert_eq!(
            next(after_gap, &at("2026-03-27T12:00:00")),
            Some(at("2026-03-29T04:00:00"))
        );
    }

    /// A server Core takes to be running, and the lock that keeps it so.
    async fn running()
    -> (tempfile::TempDir, Arc<Servers>, String, std::fs::File) {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::daemon::paths::Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let servers = Servers::open(paths, db::memory().await).await.unwrap();
        let id = servers.create("A").await.unwrap().id;
        let lock = servers.pretend_running(&id);
        (dir, servers, id, lock)
    }

    #[tokio::test]
    async fn a_countdown_belongs_to_one_run_and_its_settings() {
        let (_dir, servers, id, lock) = running().await;
        let bound = Bound::now(&servers, &id).await.unwrap();
        assert!(holds(&servers, &id, Why::Schedule, &bound).await);
        // The settings changed: a new countdown follows them, this one stops.
        let daily = Restarts {
            schedule: Some(Schedule::Daily {
                at: "04:00".parse().unwrap(),
            }),
            ..Restarts::default()
        };
        set(&servers.db, &id, &daily).await.unwrap();
        assert!(!holds(&servers, &id, Why::Schedule, &bound).await);
        set(&servers.db, &id, &Restarts::default()).await.unwrap();
        assert!(holds(&servers, &id, Why::Schedule, &bound).await);
        // Another run of the same server is not the run it was for.
        drop(lock);
        let _lock = servers.pretend_running(&id);
        assert!(!holds(&servers, &id, Why::Schedule, &bound).await);
        // A smart restart also stops once someone is online.
        let bound = Bound::now(&servers, &id).await.unwrap();
        assert!(holds(&servers, &id, Why::Smart, &bound).await);
        servers.pretend_online(&id, "Steve");
        assert!(!holds(&servers, &id, Why::Smart, &bound).await);
        assert!(holds(&servers, &id, Why::Schedule, &bound).await);
    }

    #[tokio::test]
    async fn a_countdown_that_no_longer_holds_ends_without_a_restart() {
        let (_dir, servers, id, _lock) = running().await;
        let bound = Bound::now(&servers, &id).await.unwrap();
        // One that holds keeps counting down.
        let waiting = tokio::time::timeout(
            Duration::from_millis(1500),
            countdown(&servers, &id, now() + 300, Why::Schedule, &bound),
        )
        .await;
        assert!(waiting.is_err());
        set(
            &servers.db,
            &id,
            &Restarts {
                crash: false,
                ..Restarts::default()
            },
        )
        .await
        .unwrap();
        let ended = tokio::time::timeout(
            Duration::from_secs(3),
            countdown(&servers, &id, now() + 300, Why::Schedule, &bound),
        )
        .await;
        assert!(ended.is_ok());
        assert_eq!(servers.phase(&id), Some(Phase::Running), "not restarted");
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
