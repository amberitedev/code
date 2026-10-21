//! What Core learns from a running server's console: who is online, and whether it still
//! answers.
//!
//! The console is untrusted text: a player's chat or a mod can print anything. Join and leave
//! lines count only when the whole message after the log header matches, so chat cannot fake
//! them, and `list` corrects the count every few minutes. A list line counts only while Core's
//! own `list` waits for its answer. Freeze detection sends `scoreboard players get copal <word>`
//! with a random word. Minecraft answers `Unknown scoreboard objective '<word>'`, which nothing
//! else can print without knowing the word. Each word answers once, and only soon after it was
//! sent.
//!
//! All of this is still the console's word, so what Core does with it is advisory: a smart
//! restart warns players first (see `restarts`).

use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

/// How often Core asks a server for the random word.
pub const PROBE_EVERY: Duration = Duration::from_secs(60);
/// A server that answered before and has not answered for this long is frozen.
pub const FROZEN_AFTER: Duration = Duration::from_secs(180);
/// How often Core asks for `list`, to correct what join and leave lines said.
pub const LIST_EVERY: Duration = Duration::from_secs(5 * 60);
/// How long after asking a `list` answer counts.
const LIST_ANSWER_WITHIN: Duration = Duration::from_secs(10);
/// How long after sending a probe word its answer counts.
const PROBE_ANSWER_WITHIN: Duration = PROBE_EVERY;
/// The most players Core tracks, whatever the console claims.
const MAX_PLAYERS: usize = 1000;

/// What a running server's console told Core so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Insight {
    pub players: BTreeSet<String>,
    /// `list` answered at least once this run, so `players` is checked.
    pub players_checked: bool,
    /// Unix seconds of the last probe answer. `None` until the first, and while freeze
    /// detection cannot tell, such as on a server whose console does not echo the probe.
    pub answered_at: Option<i64>,
    pub frozen: bool,
    /// The server printed its "Done" line.
    pub ready: bool,
    /// When Core last sent `list`, until its answer arrives.
    pub(super) list_asked: Option<Instant>,
}

/// One console line, as Core reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Ready,
    Joined(String),
    Left(String),
    List(Vec<String>),
    Other,
}

/// The probe command for `word`.
pub fn probe(word: &str) -> String {
    format!("scoreboard players get copal {word}")
}

/// A random lowercase word for a probe.
pub fn word() -> String {
    uuid::Uuid::new_v4()
        .as_bytes()
        .iter()
        .take(10)
        .map(|byte| char::from(b'a' + byte % 26))
        .collect()
}

/// The probe words sent and not answered yet.
#[derive(Debug, Default)]
pub struct Probes {
    sent: Vec<(String, Instant)>,
}

impl Probes {
    /// A probe command with a new word, sent `at`.
    pub fn send(&mut self, at: Instant) -> String {
        self.sent.retain(|(_, sent)| {
            at.duration_since(*sent) <= PROBE_ANSWER_WITHIN
        });
        let word = word();
        let command = probe(&word);
        self.sent.push((word, at));
        command
    }

    /// Whether `line`, read `at`, is Minecraft's answer to a word still waiting. The word is
    /// used up.
    pub fn answered(&mut self, line: &str, at: Instant) -> bool {
        let Some(word) = message(line)
            .and_then(|message| {
                message.strip_prefix("Unknown scoreboard objective '")
            })
            .and_then(|rest| rest.strip_suffix('\''))
        else {
            return false;
        };
        let Some(index) = self.sent.iter().position(|(sent, when)| {
            sent == word && at.duration_since(*when) <= PROBE_ANSWER_WITHIN
        }) else {
            return false;
        };
        self.sent.swap_remove(index);
        true
    }
}

impl Insight {
    /// Core sent `list` `at`: its answer counts for a short while.
    pub fn ask_list(&mut self, at: Instant) {
        self.list_asked = Some(at);
    }

    /// Take in one console line, read `at`.
    pub fn read(&mut self, line: &str, at: Instant) {
        match classify(line) {
            Line::Ready => self.ready = true,
            Line::Joined(name) if self.players.len() < MAX_PLAYERS => {
                self.players.insert(name);
            }
            Line::Left(name) => {
                self.players.remove(&name);
            }
            Line::List(names)
                if self.list_asked.take().is_some_and(|asked| {
                    at.duration_since(asked) <= LIST_ANSWER_WITHIN
                }) =>
            {
                self.players = names.into_iter().take(MAX_PLAYERS).collect();
                self.players_checked = true;
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

    #[test]
    fn joins_leaves_and_list_are_read() {
        let at = Instant::now();
        let mut insight = Insight::default();
        insight.read(&line("Steve joined the game"), at);
        insight.read(&line("Alex joined the game"), at);
        insight.read(&line("Steve left the game"), at);
        assert_eq!(insight.players, BTreeSet::from(["Alex".into()]));
        assert!(!insight.players_checked);
        insight.ask_list(at);
        insight.read(
            &line("There are 2 of a max of 20 players online: Alex, Notch"),
            at,
        );
        assert_eq!(
            insight.players,
            BTreeSet::from(["Alex".into(), "Notch".into()])
        );
        assert!(insight.players_checked);
        insight.ask_list(at);
        insight.read(&line("There are 0 of a max of 20 players online: "), at);
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
        let at = Instant::now();
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
            // A list that disagrees with itself, or names nobody could have.
            line("There are 3 of a max of 20 players online: Alex, Notch"),
            line("There are 1 of a max of 20 players online: Al ex"),
            line("There are x of a max of 20 players online: Alex"),
        ] {
            let mut insight = Insight::default();
            insight.read(&hostile, at);
            assert_eq!(insight, Insight::default(), "{hostile}");
        }
        // Lines are capped long before they reach here, but a long one still does nothing.
        let mut insight = Insight::default();
        insight.read(&line(&"A".repeat(70_000)), at);
        assert_eq!(insight, Insight::default());
    }

    #[test]
    fn a_list_counts_only_as_the_answer_to_cores_own() {
        let at = Instant::now();
        let empty = line("There are 0 of a max of 20 players online: ");
        let mut insight = Insight::default();
        insight.read(&line("Steve joined the game"), at);
        // Printed by a mod with nobody asking: not believed.
        insight.read(&empty, at);
        assert!(!insight.players_checked);
        assert_eq!(insight.players.len(), 1);
        // An answer that comes too late does not count, and uses up the question.
        insight.ask_list(at);
        insight.read(&empty, at + Duration::from_secs(11));
        assert!(!insight.players_checked);
        insight.read(&empty, at + Duration::from_secs(12));
        assert!(!insight.players_checked);
        // One answer per question.
        insight.ask_list(at);
        insight.read(&empty, at + Duration::from_secs(1));
        assert!(insight.players_checked && insight.players.is_empty());
        insight.read(&line("Steve joined the game"), at);
        insight.read(&empty, at + Duration::from_secs(2));
        assert_eq!(insight.players.len(), 1);
    }

    /// The word in a probe command.
    fn word_of(command: &str) -> &str {
        command.rsplit(' ').next().unwrap()
    }

    fn answer(word: &str) -> String {
        line(&format!("Unknown scoreboard objective '{word}'"))
    }

    #[test]
    fn only_the_random_word_answers_a_probe_and_only_once() {
        let at = Instant::now();
        let mut probes = Probes::default();
        let first = probes.send(at);
        let second = probes.send(at);
        assert!(first.starts_with("scoreboard players get copal "));
        let (first, second) = (word_of(&first), word_of(&second));
        assert_eq!(first.len(), 10);
        assert_ne!(first, second);
        // Someone guessing, the word in chat or without the header, or the word in another
        // message.
        for wrong in [
            answer("aaaaaaaaaa"),
            line(&format!("<Mallory> Unknown scoreboard objective '{first}'")),
            format!("Unknown scoreboard objective '{first}'"),
            line(&format!("I saw '{first}'")),
            line(&format!("Unknown scoreboard objective '{first}' indeed")),
        ] {
            assert!(!probes.answered(&wrong, at), "{wrong}");
        }
        assert!(probes.answered(&answer(second), at));
        assert!(!probes.answered(&answer(second), at), "a word answers once");
        assert!(probes.answered(&answer(first), at));
    }

    #[test]
    fn a_late_answer_does_not_count() {
        let at = Instant::now();
        let mut probes = Probes::default();
        let command = probes.send(at);
        let word = word_of(&command);
        assert!(!probes.answered(&answer(word), at + Duration::from_secs(61)));
        // Sending again forgets the words that can no longer count.
        let command = probes.send(at);
        let late = at + Duration::from_secs(120);
        probes.send(late);
        assert!(!probes.answered(&answer(word_of(&command)), late));
        assert_eq!(probes.sent.len(), 1);
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
        insight.answered_at = Some(1_200);
        insight.check_frozen(1_200);
        assert!(!insight.frozen, "answering again clears it");
    }
}
