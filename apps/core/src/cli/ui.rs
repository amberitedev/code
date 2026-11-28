//! How every command talks to a person. In a terminal it draws with `cliclack`: step lines,
//! spinners, bars, and lists picked with the arrow keys. Anywhere else (a pipe, a script, an
//! agent) it prints plain lines and never asks: a missing answer fails and names the flag that
//! gives it. With `--json` it prints nothing, and the command's JSON is the whole output.

use std::{cell::Cell, fmt::Display, io::IsTerminal};

use color_eyre::eyre::{Result, bail};
use console::style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Terminal,
    Plain,
    Json,
}

pub struct Ui {
    mode: Mode,
    /// A frame is open: `intro` drew its top and nothing closed it yet.
    open: Cell<bool>,
}

/// The person left a prompt with Esc or Ctrl+C. The screen already says so.
#[derive(Debug, thiserror::Error)]
#[error("cancelled")]
pub struct Cancelled;

/// How a line is marked in a terminal. Plain output prints the text alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Something that is done or true.
    Done,
    /// A fact, neither good nor bad.
    Info,
    Warning,
    Problem,
}

impl Ui {
    pub fn new(json: bool) -> Self {
        let terminal = std::io::stdin().is_terminal()
            && std::io::stdout().is_terminal()
            && std::io::stderr().is_terminal();
        let mode = match (json, terminal) {
            (true, _) => Mode::Json,
            (false, true) => Mode::Terminal,
            (false, false) => Mode::Plain,
        };
        // Styled text is for a terminal only, whichever stream it ends up on.
        if mode != Mode::Terminal {
            console::set_colors_enabled(false);
            console::set_colors_enabled_stderr(false);
        }
        Self {
            mode,
            open: Cell::new(false),
        }
    }

    /// A person is looking and can answer.
    pub fn terminal(&self) -> bool {
        self.mode == Mode::Terminal
    }

    /// The top of a command's frame, naming the command.
    pub fn intro(&self, command: &str) {
        if self.terminal() {
            let title = style(format!(" {command} ")).on_cyan().black();
            let _ = cliclack::intro(title);
            self.open.set(true);
        }
    }

    /// One line or block of the command's output.
    pub fn say(&self, mark: Mark, text: impl Display) {
        match self.mode {
            Mode::Terminal => {
                let text = wrapped(text);
                let _ = match mark {
                    Mark::Done => cliclack::log::step(text),
                    Mark::Info => cliclack::log::info(text),
                    Mark::Warning => cliclack::log::warning(text),
                    Mark::Problem => cliclack::log::error(text),
                };
            }
            Mode::Plain => println!("{text}"),
            Mode::Json => {}
        }
    }

    /// A titled block, such as a list of steps to follow.
    pub fn note(&self, title: &str, body: impl Display) {
        match self.mode {
            Mode::Terminal => {
                let _ = cliclack::note(title, body);
            }
            Mode::Plain => println!("{title}\n{body}"),
            Mode::Json => {}
        }
    }

    /// The bottom of the frame: what to do next, or that it is done.
    pub fn outro(&self, text: impl Display) {
        match self.mode {
            Mode::Terminal => {
                let _ = cliclack::outro(under_the_corner(text));
                self.open.set(false);
            }
            Mode::Plain => println!("{text}"),
            Mode::Json => {}
        }
    }

    /// Close the frame on an error: what is wrong, then the next step where every command puts
    /// its next step. False when no frame is open, so the caller prints it.
    pub fn abort(&self, error: impl Display, next: Option<&str>) -> bool {
        if !self.open.replace(false) {
            return false;
        }
        let _ = cliclack::log::error(wrapped(error));
        let _ = match next {
            Some(next) => cliclack::outro(under_the_corner(next)),
            None => cliclack::outro_cancel(""),
        };
        true
    }

    /// Show that something is happening until it is stopped. Nothing shows outside a terminal.
    pub fn spinner(&self, text: impl Display) -> Spinner {
        Spinner {
            bar: self.terminal().then(|| {
                let bar = cliclack::spinner();
                bar.start(text);
                bar
            }),
            plain: self.mode == Mode::Plain,
        }
    }

    /// A bar that fills over `steps` steps.
    pub fn progress(&self, steps: u64, text: impl Display) -> Spinner {
        Spinner {
            bar: self.terminal().then(|| {
                let bar = cliclack::progress_bar(steps);
                bar.start(text);
                bar
            }),
            plain: self.mode == Mode::Plain,
        }
    }

    /// Ask yes or no, No in focus. `flag` is the flag that answers yes.
    pub fn confirm(&self, question: &str, flag: &str) -> Result<bool> {
        self.ask(question, flag, false)
    }

    /// Ask yes or no with Yes in focus, for what the person just asked to have done.
    pub fn confirm_expected(&self, question: &str, flag: &str) -> Result<bool> {
        self.ask(question, flag, true)
    }

    fn ask(&self, question: &str, flag: &str, leaning: bool) -> Result<bool> {
        if !self.terminal() {
            bail!(
                "this asks \"{question}\" and there is no terminal to ask in. Pass {flag} to say yes"
            );
        }
        self.asked(
            cliclack::confirm(question)
                .initial_value(leaning)
                .interact(),
        )
    }

    /// Pick one of `items`, each a value, its label and a hint. `how` says how to name the
    /// answer on the command line instead.
    pub fn select<T: Clone + Eq>(
        &self,
        question: &str,
        items: &[(T, String, String)],
        how: &str,
    ) -> Result<T> {
        if !self.terminal() {
            bail!(
                "this asks \"{question}\" and there is no terminal to ask in. {how}"
            );
        }
        self.asked(cliclack::select(question).items(items).interact())
    }

    /// Tick any of `items`, all ticked to begin with. `how` as in `select`.
    pub fn multiselect<T: Clone + Eq>(
        &self,
        question: &str,
        items: &[(T, String, String)],
        how: &str,
    ) -> Result<Vec<T>> {
        if !self.terminal() {
            bail!(
                "this asks \"{question}\" and there is no terminal to ask in. {how}"
            );
        }
        let all = items.iter().map(|(value, ..)| value.clone()).collect();
        self.asked(
            cliclack::multiselect(question)
                .items(items)
                .initial_values(all)
                .required(false)
                .interact(),
        )
    }

    /// A prompt's answer. Leaving the prompt closes the frame.
    fn asked<T>(&self, answer: std::io::Result<T>) -> Result<T> {
        match answer {
            Ok(answer) => Ok(answer),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                self.abort("Cancelled. Nothing was changed.", None);
                Err(Cancelled.into())
            }
            Err(error) => Err(error.into()),
        }
    }
}

/// A spinner or a progress bar in a terminal, and nothing elsewhere but its last line.
pub struct Spinner {
    bar: Option<cliclack::ProgressBar>,
    plain: bool,
}

impl Spinner {
    /// Say what is happening now.
    pub fn message(&self, text: impl Display) {
        if let Some(bar) = &self.bar {
            bar.set_message(text);
        }
    }

    /// One step of a progress bar is done.
    pub fn step(&self) {
        if let Some(bar) = &self.bar {
            bar.inc(1);
        }
    }

    /// It worked: leave `text` in its place.
    pub fn done(self, text: impl Display) {
        match self.bar {
            Some(bar) => bar.stop(text),
            None if self.plain => println!("{text}"),
            None => {}
        }
    }

    /// It did not work: leave `text` in its place, marked as an error.
    pub fn failed(self, text: impl Display) {
        match self.bar {
            Some(bar) => bar.cancel(text),
            None if self.plain => eprintln!("{text}"),
            None => {}
        }
    }

    /// Remove it without a trace.
    pub fn clear(self) {
        if let Some(bar) = self.bar {
            bar.clear();
        }
    }
}

/// `text` with its long lines folded to fit inside the frame, so none runs through its edge.
/// What is folded over goes a little further in than its line began, so a list still reads as
/// one.
fn wrapped(text: impl Display) -> String {
    // The frame's bar and its gap, and a column to spare.
    let width = usize::from(console::Term::stderr().size().1).saturating_sub(4);
    commands(&text.to_string())
        .lines()
        .map(|line| fold(line, width.max(20)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `line` folded to the terminal's width, what runs over set in from the left. For output
/// that has no frame, such as a log.
pub fn folded(line: &str) -> String {
    let width = usize::from(console::Term::stdout().size().1);
    fold(line, width.max(20))
}

/// The last line of a frame, folded so what runs over stays under its first word. Nothing
/// draws the frame's bar there.
fn under_the_corner(text: impl Display) -> String {
    wrapped(text).replace('\n', "\n   ")
}

/// `text` beginning with a capital, as a sentence shown to a person does.
pub fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `text` with every command it names standing out: `copal`, its subcommand, and the flags
/// that follow. A next step is the thing to find on a screen.
fn commands(text: &str) -> String {
    const NAMES: [&str; 12] = [
        "status",
        "doctor",
        "pair",
        "unpair",
        "list",
        "start",
        "stop",
        "storage",
        "logs",
        "update",
        "uninstall",
        "setup",
    ];
    let bare = |word: &str| -> String {
        word.trim_end_matches(['.', ',', ':', ')']).to_owned()
    };
    let mut out = Vec::new();
    for line in text.split('\n') {
        let mut words: Vec<String> =
            line.split(' ').map(str::to_owned).collect();
        let mut index = 0;
        while index < words.len() {
            if words[index] != "copal" {
                index += 1;
                continue;
            }
            // `copal`, then a subcommand, then any flags.
            let mut end = index;
            if words
                .get(index + 1)
                .is_some_and(|next| NAMES.contains(&bare(next).as_str()))
            {
                end += 1;
            }
            while words
                .get(end + 1)
                .is_some_and(|next| next.starts_with("--"))
            {
                end += 1;
            }
            let last = bare(&words[end]);
            let after = words[end][last.len()..].to_owned();
            let named = [&words[index..end], &[last]].concat().join(" ");
            words.splice(index..=end, [format!("{}{after}", cyan(named))]);
            index += 1;
        }
        out.push(words.join(" "));
    }
    out.join("\n")
}

fn fold(line: &str, width: usize) -> String {
    if console::measure_text_width(line) <= width {
        return line.to_owned();
    }
    let indent = line.len() - line.trim_start().len();
    // Under the text of a list's line, past its mark.
    let hang = " ".repeat(indent + 2);
    let mut lines = vec![line[..indent].to_owned()];
    // Split on single spaces, so the gaps that line columns up stay as wide as they were.
    for (index, word) in line[indent..].split(' ').enumerate() {
        let current = lines.last_mut().expect("starts with one line");
        let fits = console::measure_text_width(current)
            + console::measure_text_width(word)
            < width;
        if index == 0 {
            current.push_str(word);
        } else if fits {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(format!("{hang}{word}"));
        }
        // One word wider than the line, such as a long path, is cut where the line ends.
        while let Some(last) = lines.last().filter(|last| {
            // Only plain text can be cut by its bytes.
            last.is_ascii()
                && !last.contains('\u{1b}')
                && last.len() > width
                && width > hang.len()
        }) {
            let (head, tail) = last.split_at(width);
            let (head, tail) = (head.to_owned(), format!("{hang}{tail}"));
            lines.pop();
            lines.extend([head, tail]);
        }
    }
    lines.join("\n")
}

/// `fancy` where the terminal draws it, `plain` in an old Windows console that cannot. The
/// rule `cliclack` goes by for its own symbols.
pub fn glyph(fancy: &str, plain: &str) -> String {
    console::Emoji(fancy, plain).to_string()
}

/// Dim text, for what matters less. Unstyled outside a terminal.
pub fn dim(text: impl Display) -> String {
    style(text).dim().to_string()
}

pub fn bold(text: impl Display) -> String {
    style(text).bold().to_string()
}

pub fn green(text: impl Display) -> String {
    style(text).green().to_string()
}

pub fn yellow(text: impl Display) -> String {
    style(text).yellow().to_string()
}

pub fn red(text: impl Display) -> String {
    style(text).red().to_string()
}

pub fn cyan(text: impl Display) -> String {
    style(text).cyan().to_string()
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn a_long_line_folds_under_itself_and_keeps_its_place_in_a_list() {
        assert_eq!(fold("short", 20), "short");
        assert_eq!(
            fold("  -> forward the port in the router", 20),
            "  -> forward the\n    port in the\n    router"
        );
        assert_eq!(
            fold(r"in C:\ProgramData\Copal\servers", 16),
            "in\n  C:\\ProgramData\n  \\Copal\\servers"
        );
    }
}
