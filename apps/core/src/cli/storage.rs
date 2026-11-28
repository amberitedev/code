//! `copal storage`: what takes up space on this computer's disk.

use color_eyre::eyre::Result;
use contracts::{
    Empty,
    core::{DiskUsage, GetDiskUsage, ServerUsage},
};
use sharing_client::disk::label;

use super::{
    Client, Done,
    call::call,
    ui::{self, Mark, Tone, Ui},
};

/// How many servers are named without `--all`.
const BIGGEST: usize = 5;
/// The width of a bar, in cells.
const BAR: usize = 48;

/// One part of a whole, with the colour it is drawn in.
struct Part {
    name: &'static str,
    bytes: u64,
    tone: Tone,
}

/// `copal storage`.
pub async fn run(ui: &Ui, client: &Client, all: bool) -> Result<Done> {
    ui.begin();
    let spinner = ui.spinner("Measuring what is on the disk");
    let usage = match call::<GetDiskUsage>(client, &(), &(), Empty).await {
        Ok(usage) => usage,
        Err(error) => {
            spinner.clear();
            return Err(error.wrap_err("Could not measure the disk"));
        }
    };
    spinner.clear();
    let kinds = kinds(&usage);
    let core: u64 = kinds.iter().map(|kind| kind.bytes).sum();
    let used = usage.total.saturating_sub(usage.free);
    let disk = [
        Part {
            name: "Core",
            bytes: core.min(used),
            tone: Tone::Accent,
        },
        Part {
            name: "Everything else",
            bytes: used.saturating_sub(core),
            tone: Tone::Grey,
        },
        Part {
            name: "Free",
            bytes: usage.free,
            tone: Tone::Faint,
        },
    ];
    // Through its parts, so the separators are this OS's own.
    let folder: std::path::PathBuf =
        crate::daemon::config::data_dir().components().collect();
    let folder = folder.display().to_string();
    let servers = biggest(&usage, all);
    let more = usage.servers.len() - servers.len();

    if ui.terminal() {
        ui.say(
            Mark::Done,
            format!(
                "Disk {}\n{}",
                ui::dim(size(usage.total)),
                breakdown(&disk, usage.total)
            ),
        );
        ui.say(
            Mark::Done,
            format!(
                "Core {}\n{}\n{} {folder}",
                ui::dim(size(core)),
                breakdown(&kinds, core),
                // The path is left plain, so a long one can be folded.
                ui::dim("in"),
            ),
        );
        if !servers.is_empty() {
            let mut rows = table(&servers);
            if more > 0 {
                rows.push(ui::dim(format!(
                    "and {more} smaller, with: copal storage --all"
                )));
            }
            ui.say(Mark::Done, rows.join("\n"));
        }
        ui.outro("Free space by deleting backups or servers in Amberite.");
    } else {
        let mut lines = vec![format!(
            "Disk: {} used of {}, {} free",
            size(used),
            size(usage.total),
            size(usage.free)
        )];
        lines.push(format!("Core uses {}", size(core)));
        lines.extend(
            kinds
                .iter()
                .map(|kind| format!("{}: {}", kind.name, size(kind.bytes))),
        );
        lines.push(format!("Data folder: {folder}"));
        lines.extend(servers.iter().map(|server| {
            format!(
                "{}: {} (files {}, backups {} in {})",
                super::safe(&server.name),
                size(server.files + server.backups),
                size(server.files),
                size(server.backups),
                server.backup_count
            )
        }));
        if more > 0 {
            lines.push(format!("and {more} smaller: copal storage --all"));
        }
        ui.say(Mark::Done, lines.join("\n"));
    }
    Ok(Done::ok(&usage))
}

/// What Core keeps, by kind, biggest first.
fn kinds(usage: &DiskUsage) -> Vec<Part> {
    let sum = |bytes: fn(&ServerUsage) -> u64| {
        usage.servers.iter().map(bytes).sum::<u64>()
    };
    let mut kinds = [
        ("Servers", sum(|server| server.files)),
        ("Backups", sum(|server| server.backups)),
        ("Minecraft and Java", usage.game_files),
        ("Shared files", usage.shared),
    ];
    kinds.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
    // The biggest is the brand's own colour, and each one after it a shade further off.
    let tones = [Tone::Accent, Tone::Soft, Tone::Pale, Tone::Grey];
    kinds
        .into_iter()
        .zip(tones)
        .map(|((name, bytes), tone)| Part { name, bytes, tone })
        .collect()
}

/// The servers that take the most, biggest first: all of them with `all`, otherwise the
/// first few that are worth a megabyte.
fn biggest(usage: &DiskUsage, all: bool) -> Vec<&ServerUsage> {
    let mut servers: Vec<&ServerUsage> = usage.servers.iter().collect();
    servers
        .sort_by_key(|server| std::cmp::Reverse(server.files + server.backups));
    if !all {
        servers.retain(|server| server.files + server.backups >= 1024 * 1024);
        servers.truncate(BIGGEST);
    }
    servers
}

/// One bar for the whole, then each part under it with its colour, its size and its share.
fn breakdown(parts: &[Part], whole: u64) -> String {
    let sizes: Vec<u64> = parts.iter().map(|part| part.bytes).collect();
    let bar: String = parts
        .iter()
        .zip(cells(&sizes, whole, BAR))
        .map(|(part, cells)| {
            ui::paint(ui::glyph("━", "=").repeat(cells), part.tone)
        })
        .collect();
    let width = parts.iter().map(|part| part.name.len()).max().unwrap_or(0);
    let mut lines = vec![bar];
    lines.extend(parts.iter().map(|part| {
        let share = if whole == 0 {
            0.0
        } else {
            part.bytes as f64 / whole as f64 * 100.0
        };
        format!(
            "{} {:<width$}  {:>10}  {}",
            ui::paint(ui::glyph("■", "#"), part.tone),
            part.name,
            size(part.bytes),
            ui::dim(if part.bytes > 0 && share < 1.0 {
                " <1%".to_owned()
            } else {
                format!("{share:>3.0}%")
            })
        )
    }));
    lines.join("\n")
}

/// A row per server: what its files take, what its backups take, and both.
fn table(servers: &[&ServerUsage]) -> Vec<String> {
    let names: Vec<String> = servers
        .iter()
        .map(|server| super::safe(&server.name))
        .collect();
    let width = names
        .iter()
        .map(|name| name.chars().count())
        .chain(["Servers".len()])
        .max()
        .unwrap_or(0);
    let mut rows = vec![format!(
        "{:<width$}  {}",
        "Servers",
        ui::dim(format!(
            "{:>10}  {:>16}  {:>10}",
            "FILES", "BACKUPS", "TOTAL"
        ))
    )];
    rows.extend(servers.iter().zip(names).map(|(server, name)| {
        let backups = match server.backup_count {
            0 => ui::dim(format!("{:>16}", "none")),
            count => {
                format!("{:>16}", format!("{} ({count})", size(server.backups)))
            }
        };
        format!(
            "{name:<width$}  {:>10}  {backups}  {:>10}",
            size(server.files),
            size(server.files + server.backups),
        )
    }));
    rows
}

/// A size as a person reads it. `label` alone rounds everything under a megabyte up to one.
fn size(bytes: u64) -> String {
    match bytes {
        0 => "nothing".into(),
        bytes if bytes < 1024 * 1024 => "under 1 MB".into(),
        bytes => label(bytes),
    }
}

/// How many of `width` cells each of `parts` of `whole` fills. Anything worth a megabyte gets
/// at least one, and together they never fill more than the bar.
fn cells(parts: &[u64], whole: u64, width: usize) -> Vec<usize> {
    let mut cells: Vec<usize> = parts
        .iter()
        .map(|&part| {
            if part < 1024 * 1024 || whole == 0 {
                return 0;
            }
            let share = part as f64 / whole as f64 * width as f64;
            (share.round() as usize).clamp(1, width)
        })
        .collect();
    // Rounding up the small ones can overflow: the biggest gives the cells back.
    while cells.iter().sum::<usize>() > width {
        let biggest = cells
            .iter_mut()
            .max_by_key(|cells| **cells)
            .expect("an overflow has cells");
        *biggest -= 1;
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * MB;

    fn usage() -> DiskUsage {
        let server = |name: &str, files, backups, backup_count| ServerUsage {
            id: name.to_lowercase(),
            name: name.into(),
            files,
            backups,
            backup_count,
        };
        DiskUsage {
            total: 100 * GB,
            free: 40 * GB,
            servers: vec![
                server("Creative", 5 * GB, 0, 0),
                server("Survival", 3 * GB, 9 * GB, 4),
                server("Empty", 165, 0, 0),
            ],
            shared: GB,
            game_files: 2 * GB,
        }
    }

    #[test]
    fn the_biggest_come_first_and_the_trivial_wait_for_all() {
        let usage = usage();
        let named = |all| -> Vec<&str> {
            biggest(&usage, all)
                .iter()
                .map(|server| server.name.as_str())
                .collect()
        };
        assert_eq!(named(false), ["Survival", "Creative"]);
        assert_eq!(named(true), ["Survival", "Creative", "Empty"]);
        let kinds: Vec<&str> =
            kinds(&usage).iter().map(|kind| kind.name).collect();
        assert_eq!(
            kinds,
            ["Backups", "Servers", "Minecraft and Java", "Shared files"]
        );
    }

    #[test]
    fn a_size_is_never_rounded_up_to_a_megabyte() {
        assert_eq!(size(0), "nothing");
        assert_eq!(size(165), "under 1 MB");
        assert_eq!(size(74 * MB), "74 MB");
        assert_eq!(size(1024 * GB), "1.0 TB");
    }

    #[test]
    fn a_bar_shows_anything_worth_showing_and_never_overflows() {
        assert_eq!(cells(&[0, 165, 50 * MB], 100 * MB, 10), [0, 0, 5]);
        // Three slivers and the rest: each sliver shows, and the bar is still ten wide.
        assert_eq!(cells(&[MB, MB, MB, 997 * MB], 1000 * MB, 10), [1, 1, 1, 7]);
        assert_eq!(cells(&[200 * MB], 100 * MB, 10), [10]);
    }
}
