//! How every command talks to a person. In a terminal it draws with `cliclack`: step lines,
//! spinners, bars, and lists picked with the arrow keys. Anywhere else (a pipe, a script, an
//! agent) it prints plain lines and never asks: a missing answer fails and names the flag that
//! gives it. With `--json` it prints nothing, and the command's JSON is the whole output.

use std::{cell::Cell, fmt::Display, io::IsTerminal};

use cliclack::ThemeState;
use color_eyre::eyre::{Result, bail};
use console::{Style, style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Terminal,
    Plain,
    Json,
}

pub struct Ui {
    mode: Mode,
    /// A command began drawing and nothing closed its frame yet.
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
        if mode == Mode::Terminal {
            cliclack::set_theme(Amber);
        } else {
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

    /// The help text: every command and what it is for. With `--json` it is not shown.
    pub fn help(&self, help: clap::builder::StyledStr) {
        match self.mode {
            Mode::Terminal => println!("{}", help.ansi()),
            Mode::Plain => println!("{help}"),
            Mode::Json => {}
        }
    }

    /// A command's output begins: from here an error closes the frame it is drawn in.
    pub fn begin(&self) {
        if self.terminal() {
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

    /// Close the frame on an error. With a next step, what is wrong and then the step, where
    /// every command puts its next step; without one, the error is the frame's last line. False
    /// when no frame is open, so the caller prints it.
    pub fn abort(&self, error: impl Display, next: Option<&str>) -> bool {
        if !self.open.replace(false) {
            return false;
        }
        let _ = match next {
            Some(next) => cliclack::log::error(wrapped(error))
                .and_then(|()| cliclack::outro(under_the_corner(next))),
            None => cliclack::outro_cancel(under_the_corner(error)),
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

/// `text` with every command it names standing out, since a next step is the thing to find on
/// a screen. A step is worded `... with: <command>`, so what follows `with: ` is one, whatever
/// program it runs; elsewhere it is `copal` with its subcommand and flags. Each word is
/// coloured by itself, so a command that is folded over two lines colours nothing between.
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
    // The comma between two commands belongs to neither.
    let coloured = |command: &str| -> String {
        command
            .split(' ')
            .map(|word| match word.strip_suffix(',') {
                Some(word) => format!("{},", paint(word, Tone::Accent)),
                None => paint(word, Tone::Accent),
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let bare = |word: &str| -> String {
        word.trim_end_matches(['.', ',', ':', ')']).to_owned()
    };
    let line = |line: &str| -> String {
        if let Some((before, after)) = line.split_once("with: ") {
            // Up to what is said about the command, such as that it needs an admin.
            let (command, rest) = match after.split_once(" (") {
                Some((command, rest)) => (command, format!(" ({rest}")),
                None => (after, String::new()),
            };
            return format!("{before}with: {}{rest}", coloured(command));
        }
        let mut words: Vec<String> =
            line.split(' ').map(str::to_owned).collect();
        let mut index = 0;
        while index < words.len() {
            let named = words[index] == "copal"
                && words.get(index + 1).is_some_and(|next| {
                    NAMES.contains(&bare(next).as_str())
                        || next.starts_with("--")
                });
            if !named {
                index += 1;
                continue;
            }
            // The subcommand, when there is one, then any flags.
            let mut end = index + 1;
            while words
                .get(end + 1)
                .is_some_and(|next| next.starts_with("--"))
            {
                end += 1;
            }
            let last = bare(&words[end]);
            let after = words[end][last.len()..].to_owned();
            let command = [&words[index..end], &[last]].concat().join(" ");
            words.splice(
                index..=end,
                [format!("{}{after}", coloured(&command))],
            );
            index += 1;
        }
        words.join(" ")
    };
    text.split('\n').map(line).collect::<Vec<_>>().join("\n")
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

/// The colours the CLI draws in: Amberite's orange for what leads the eye (the command's
/// name, the next step, what is picked), lighter shades of it for a second and third part of
/// a bar, and greys for the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Soft,
    Pale,
    Grey,
    Faint,
}

impl Tone {
    /// The brand's own colours where the terminal shows them exactly, the nearest of the 256
    /// where it does not.
    fn style(self) -> Style {
        let (exact, near) = match self {
            Self::Accent => ((255, 100, 26), 202),
            Self::Soft => ((255, 171, 128), 216),
            Self::Pale => ((255, 220, 190), 223),
            Self::Grey => ((150, 150, 150), 246),
            Self::Faint => ((70, 70, 70), 238),
        };
        if console::true_colors_enabled_stderr() {
            Style::new().true_color(exact.0, exact.1, exact.2)
        } else {
            Style::new().color256(near)
        }
    }
}

pub fn paint(text: impl Display, tone: Tone) -> String {
    tone.style().apply_to(text).to_string()
}

/// The help's headings in the same orange as everything else the CLI leads with.
pub fn help_styles() -> clap::builder::Styles {
    use clap::builder::styling::{Color, RgbColor, Style};

    let accent = Style::new()
        .bold()
        .fg_color(Some(Color::Rgb(RgbColor(255, 100, 26))));
    clap::builder::Styles::styled()
        .header(accent)
        .usage(accent)
        .literal(Style::new().bold())
        .placeholder(Style::new().dimmed())
}

/// How `cliclack` draws: its own layout, in Amberite's orange, with a frame whose corners are
/// round and whose rail stays out of the way.
struct Amber;

impl cliclack::Theme for Amber {
    fn bar_color(&self, state: &ThemeState) -> Style {
        match state {
            ThemeState::Active => Tone::Accent.style(),
            ThemeState::Cancel => Style::new().red(),
            ThemeState::Submit => Tone::Faint.style(),
            ThemeState::Error(_) => Style::new().yellow(),
        }
    }

    fn state_symbol_color(&self, state: &ThemeState) -> Style {
        match state {
            ThemeState::Submit => Tone::Accent.style(),
            _ => self.bar_color(state),
        }
    }

    fn radio_symbol(&self, state: &ThemeState, selected: bool) -> String {
        match state {
            ThemeState::Active if selected => {
                paint(glyph("●", ">"), Tone::Accent)
            }
            ThemeState::Active => dim(glyph("○", " ")),
            _ => String::new(),
        }
    }

    fn checkbox_symbol(
        &self,
        state: &ThemeState,
        selected: bool,
        active: bool,
    ) -> String {
        match state {
            ThemeState::Active | ThemeState::Error(_) if selected => {
                paint(glyph("◼", "[+]"), Tone::Accent)
            }
            ThemeState::Active | ThemeState::Error(_) if active => {
                paint(glyph("◻", "[ ]"), Tone::Accent)
            }
            ThemeState::Active | ThemeState::Error(_) => dim(glyph("◻", "[ ]")),
            _ => String::new(),
        }
    }

    fn info_symbol(&self) -> String {
        paint(glyph("●", "*"), Tone::Grey)
    }

    fn active_symbol(&self) -> String {
        paint(glyph("◆", "*"), Tone::Accent)
    }

    fn submit_symbol(&self) -> String {
        paint(glyph("◇", "o"), Tone::Accent)
    }

    fn format_outro(&self, message: &str) -> String {
        let rail = self.bar_color(&ThemeState::Submit);
        format!("{}  {message}\n", rail.apply_to(glyph("╰", "-")))
    }

    fn format_outro_cancel(&self, message: &str) -> String {
        let rail = self.bar_color(&ThemeState::Submit);
        format!(
            "{}  {}\n",
            rail.apply_to(glyph("╰", "-")),
            style(message).red()
        )
    }

    // A prompt someone left says nothing by itself: the frame's last line does.
    fn format_footer_with_message(
        &self,
        state: &ThemeState,
        message: &str,
    ) -> String {
        let (end, bar) = (glyph("╰", "-"), glyph("│", "|"));
        let line = match state {
            ThemeState::Active => format!("{end}  {message}"),
            ThemeState::Error(error) => format!("{end}  {error}"),
            ThemeState::Cancel | ThemeState::Submit => bar,
        };
        format!("{}\n", self.bar_color(state).apply_to(line))
    }

    fn default_progress_template(&self) -> String {
        "{msg}  {bar:24.202/238} {pos}/{len}".into()
    }

    fn format_progress_start(
        &self,
        template: &str,
        grouped: bool,
        last: bool,
    ) -> String {
        let space = if grouped { " " } else { "  " };
        self.format_progress_with_state(
            &format!("{{spinner:.202}}{space}{template}"),
            grouped,
            last,
            &ThemeState::Active,
        )
    }

    fn spinner_chars(&self) -> String {
        glyph("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏", "|/-\\")
    }

    fn progress_chars(&self) -> String {
        glyph("━╸─", "#>-")
    }
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
