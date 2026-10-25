//! What Core learns from a running server's console: who is online, and whether it still
//! answers.
//!
//! The console is untrusted text: a player's chat or a mod can print anything. Join and leave
//! lines count only when the whole message after the log header matches, so chat cannot fake
//! them. Core's health check sends `list` every [`CHECK_EVERY`]; the shim labels the answer with
//! [`HEALTH_CHECK`] in `console.log` (see `shim`), and only labeled answers count. Each one
//! corrects the player list and shows the server still answers, so the App reads its players
//! from here instead of asking the server itself.
//!
//! All of this is still the console's word, so what Core does with it is advisory: a smart
//! restart warns players first (see `restarts`).

use std::{collections::BTreeSet, time::Duration};

pub use super::console::HEALTH_CHECK;

/// How often Core sends its health check. Often enough that the App's player list is fresh.
pub const CHECK_EVERY: Duration = Duration::from_secs(10);
/// How long after a health check its `list` answer gets the label. A later answer is shown as
/// ordinary console output.
pub const ANSWER_WITHIN: Duration = CHECK_EVERY;
/// A server that answered before and has not answered for this long is frozen.
pub const FROZEN_AFTER: Duration = Duration::from_secs(180);
/// The most players Core tracks, whatever the console claims.
const MAX_PLAYERS: usize = 1000;

/// What a running server's console told Core so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Insight {
    pub players: BTreeSet<String>,
    /// The health check answered at least once this run, so `players` is checked.
    pub players_checked: bool,
    /// Unix seconds of the last health check answer. `None` until the first, and while freeze
    /// detection cannot tell, such as on a server whose `list` answers differently.
    pub answered_at: Option<i64>,
    pub frozen: bool,
    /// The server printed its "Done" line.
    pub ready: bool,
}

/// One console line, as Core reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Ready,
    Joined(String),
    Left(String),
    /// A `list` answer nobody vouched for, such as the answer to someone's own `list`.
    List(Vec<String>),
    /// The answer to Core's health check, as the shim labeled it.
    Checked(Vec<String>),
    Other,
}

impl Insight {
    /// Take in one console line, read at unix seconds `now`.
    pub fn read(&mut self, line: &str, now: i64) {
        match classify(line) {
            Line::Ready => self.ready = true,
            Line::Joined(name) if self.players.len() < MAX_PLAYERS => {
                self.players.insert(name);
            }
            Line::Left(name) => {
                self.players.remove(&name);
            }
            Line::Checked(names) => {
                self.players = names.into_iter().take(MAX_PLAYERS).collect();
                self.players_checked = true;
                self.answered_at = Some(now);
                self.frozen = false;
            }
            _ => {}
        }
    }

    /// Mark the server frozen when it answered before and not since `FROZEN_AFTER`.
    pub fn check_frozen(&mut self, now: i64) {
        self.frozen = self
            .answered_at
            .is_some_and(|at| now - at >= FROZEN_AFTER.as_secs() as i64);
    }
}

/// The message of a log line: what follows `[HH:MM:SS] [thread/LEVEL]: `, or
/// `[HH:MM:SS] [thread/LEVEL] [logger]: ` on Forge. `None` for anything else.
pub fn message(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('[')?;
    let (time, rest) = rest.split_once("] ")?;
    if time.len() != 8 || !time.bytes().all(|b| b.is_ascii_digit() || b == b':')
    {
        return None;
    }
    let rest = rest.strip_prefix('[')?;
    let (_thread, rest) = rest.split_once(']')?;
    let rest = match rest.strip_prefix(" [") {
        Some(logger) => logger.split_once(']')?.1,
        None => rest,
    };
    rest.strip_prefix(": ")
}

/// Classify a console line. Names must be valid Minecraft names and the message must be exactly
/// the line Minecraft prints.
pub fn classify(line: &str) -> Line {
    if let Some(answer) = line.strip_prefix(HEALTH_CHECK) {
        return match classify(answer) {
            Line::List(names) => Line::Checked(names),
            _ => Line::Other,
        };
    }
    let Some(message) = message(line) else {
        return Line::Other;
    };
    if message.starts_with("Done (") && message.contains("For help, type") {
        return Line::Ready;
    }
    if let Some(name) = message.strip_suffix(" joined the game")
        && is_name(name)
    {
        return Line::Joined(name.into());
    }
    if let Some(name) = message.strip_suffix(" left the game")
        && is_name(name)
    {
        return Line::Left(name.into());
    }
    // There are 2 of a max of 20 players online: Alex, Steve
    if let Some(rest) = message.strip_prefix("There are ")
        && let Some((counts, names)) = rest.split_once(" players online:")
        && let Some((count, _max)) = counts.split_once(" of a max of ")
        && let Ok(count) = count.parse::<usize>()
    {
        let names: Vec<String> = names
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        if names.len() == count && names.iter().all(|name| is_name(name)) {
            return Line::List(names);
        }
    }
    Line::Other
}

/// A Minecraft player name: 1 to 16 letters, digits, and underscores.
fn is_name(name: &str) -> bool {
    (1..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "[12:00:00] [Server thread/INFO]: ";

    fn line(message: &str) -> String {
        format!("{HEAD}{message}")
    }

    /// `list`'s answer as the shim labels it for Core's health check.
    fn checked(message: &str) -> String {
        format!("{HEALTH_CHECK}{}", line(message))
    }

    #[test]
    fn joins_leaves_and_the_health_check_are_read() {
        let mut insight = Insight::default();
        insight.read(&line("Steve joined the game"), 1);
        insight.read(&line("Alex joined the game"), 1);
        insight.read(&line("Steve left the game"), 1);
        assert_eq!(insight.players, BTreeSet::from(["Alex".into()]));
        assert!(!insight.players_checked);
        insight.read(
            &checked("There are 2 of a max of 20 players online: Alex, Notch"),
            5,
        );
        assert_eq!(
            insight.players,
            BTreeSet::from(["Alex".into(), "Notch".into()])
        );
        assert!(insight.players_checked);
        assert_eq!(insight.answered_at, Some(5));
        insight
            .read(&checked("There are 0 of a max of 20 players online: "), 6);
        assert!(insight.players.is_empty());
        // Forge puts the logger name after the thread.
        assert_eq!(
            classify(
                "[12:00:00] [Server thread/INFO] [minecraft/MinecraftServer]: Steve joined the game"
            ),
            Line::Joined("Steve".into())
        );
        assert_eq!(
            classify(&line("Done (5.723s)! For help, type \"help\"")),
            Line::Ready
        );
    }

    #[test]
    fn hostile_console_lines_are_not_believed() {
        for hostile in [
            // Chat, the server's say, and a mod's own logger name in the message.
            line("<Mallory> Steve joined the game"),
            line("[Server] Steve joined the game"),
            line("<Mallory> x]: Notch joined the game"),
            line("Not A Name joined the game"),
            line("ThisNameIsFarTooLongForMinecraft joined the game"),
            line("Steve joined the game and left"),
            line("Robert'); DROP TABLE joined the game"),
            // No header, a bad header, or the header in the middle of chat.
            "Steve joined the game".into(),
            "[xx:yy:zz] [Server thread/INFO]: Steve joined the game".into(),
            format!("{HEAD}<Mallory> {HEAD}Steve joined the game"),
            // A list nobody vouched for: someone's own `list`, or the label in chat.
            line("There are 0 of a max of 20 players online: "),
            line(&format!(
                "<Mallory> {HEALTH_CHECK}There are 0 of a max of 20 players online: "
            )),
            // A labeled line that is not a list, or labeled twice.
            checked("Steve joined the game"),
            format!(
                "{HEALTH_CHECK}{}",
                checked("There are 0 of a max of 20 players online: ")
            ),
            // A list that disagrees with itself, or names nobody could have.
            checked("There are 3 of a max of 20 players online: Alex, Notch"),
            checked("There are 1 of a max of 20 players online: Al ex"),
            checked("There are x of a max of 20 players online: Alex"),
        ] {
            let mut insight = Insight::default();
            insight.read(&hostile, 1);
            assert_eq!(insight, Insight::default(), "{hostile}");
        }
        // Lines are capped long before they reach here, but a long one still does nothing.
        let mut insight = Insight::default();
        insight.read(&line(&"A".repeat(70_000)), 1);
        assert_eq!(insight, Insight::default());
    }

    #[test]
    fn a_server_is_frozen_only_after_it_answered_once() {
        let mut insight = Insight::default();
        insight.check_frozen(10_000);
        assert!(!insight.frozen, "never answered: cannot tell");
        insight.answered_at = Some(1_000);
        insight.check_frozen(1_000 + 179);
        assert!(!insight.frozen);
        insight.check_frozen(1_000 + 180);
        assert!(insight.frozen);
        insight.read(
            &checked("There are 0 of a max of 20 players online: "),
            1_200,
        );
        assert!(!insight.frozen, "answering again clears it");
    }
}
