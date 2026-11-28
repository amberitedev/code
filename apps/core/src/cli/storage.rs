//! `copal storage`: what takes up space on this computer's disk.

use color_eyre::eyre::Result;
use contracts::{
    Empty,
    core::{DiskUsage, GetDiskUsage},
};
use sharing_client::disk::label;

use super::{
    Client, Done,
    call::call,
    ui::{self, Mark, Ui},
};

/// How many of the biggest items are named without `--all`.
const BIGGEST: usize = 5;
/// The width of every bar, in cells.
const BAR: usize = 30;

/// `copal storage`.
pub async fn run(ui: &Ui, client: &Client, all: bool) -> Result<Done> {
    ui.intro("copal storage");
    let spinner = ui.spinner("Measuring what is on the disk");
    let usage = match call::<GetDiskUsage>(client, &(), &(), Empty).await {
        Ok(usage) => usage,
        Err(error) => {
            spinner.clear();
            return Err(error.wrap_err("Could not measure the disk"));
        }
    };
    spinner.clear();
    let parts = parts(&usage);
    let core: u64 = parts.iter().map(|(_, bytes)| bytes).sum();
    let used = usage.total.saturating_sub(usage.free);
    let disk = format!(
        "Disk: {} used of {}, {} free",
        size(used),
        size(usage.total),
        size(usage.free)
    );
    // Through its parts, so the separators are this OS's own.
    let folder: std::path::PathBuf =
        crate::daemon::config::data_dir().components().collect();
    let folder = folder.display().to_string();
    let biggest = biggest(&usage);
    let shown = if all {
        biggest.len()
    } else {
        BIGGEST.min(biggest.len())
    };
    let more = biggest.len() - shown;

    if ui.terminal() {
        ui.say(
            Mark::Done,
            format!("{disk}\n{}", disk_bar(core, used, usage.total)),
        );
        let widest = parts.iter().map(|(_, bytes)| *bytes).max().unwrap_or(0);
        let rows: Vec<String> = parts
            .iter()
            .map(|(name, bytes)| {
                format!(
                    "{name:<19} {:>11}  {}",
                    size(*bytes),
                    ui::cyan(full().repeat(cells(*bytes, widest, BAR)))
                )
            })
            .collect();
        ui.say(
            Mark::Done,
            format!(
                "Core uses {}\n{}\n{}",
                size(core),
                rows.join("\n"),
                // The path is left plain, so a long one can be folded.
                format_args!("{} {folder}", ui::dim("Data folder:"))
            ),
        );
        if !biggest.is_empty() {
            let width = biggest[..shown]
                .iter()
                .map(|item| item.name.chars().count())
                .max()
                .unwrap_or(0);
            let mut rows: Vec<String> = biggest[..shown]
                .iter()
                .map(|item| {
                    format!(
                        "{:<width$} {:>11}  {}",
                        item.name,
                        size(item.bytes),
                        ui::dim(&item.what)
                    )
                })
                .collect();
            if more > 0 {
                rows.push(ui::dim(format!(
                    "and {more} smaller: copal storage --all"
                )));
            }
            ui.say(
                Mark::Done,
                format!("Biggest servers and backups\n{}", rows.join("\n")),
            );
        }
        ui.outro("Free space by deleting backups or servers in Amberite.");
    } else {
        let mut lines = vec![disk, format!("Core uses {}", size(core))];
        lines.extend(
            parts
                .iter()
                .map(|(name, bytes)| format!("{name}: {}", size(*bytes))),
        );
        lines.push(format!("Data folder: {folder}"));
        if !biggest.is_empty() {
            lines.push("Biggest servers and backups:".into());
            lines.extend(biggest[..shown].iter().map(|item| {
                format!("{}: {}, {}", item.name, size(item.bytes), item.what)
            }));
            if more > 0 {
                lines.push(format!("and {more} smaller: copal storage --all"));
            }
        }
        ui.say(Mark::Done, lines.join("\n"));
    }
    Ok(Done::ok(&usage))
}

/// A size as a person reads it. `label` alone rounds everything under a megabyte up to one.
fn size(bytes: u64) -> String {
    match bytes {
        0 => "nothing".into(),
        bytes if bytes < 1024 * 1024 => "under 1 MB".into(),
        bytes => label(bytes),
    }
}

/// What Core keeps, by kind.
fn parts(usage: &DiskUsage) -> [(&'static str, u64); 4] {
    let sum = |size: fn(&contracts::core::ServerUsage) -> u64| {
        usage.servers.iter().map(size).sum::<u64>()
    };
    [
        ("Servers", sum(|server| server.files)),
        ("Backups", sum(|server| server.backups)),
        ("Shared files", usage.shared),
        ("Minecraft and Java", usage.game_files),
    ]
}

struct Item {
    name: String,
    bytes: u64,
    what: String,
}

/// Every server and every server's backups, biggest first.
fn biggest(usage: &DiskUsage) -> Vec<Item> {
    let mut items: Vec<Item> = usage
        .servers
        .iter()
        .flat_map(|server| {
            let name = super::safe(&server.name);
            [
                Item {
                    name: name.clone(),
                    bytes: server.files,
                    what: "server".into(),
                },
                Item {
                    name,
                    bytes: server.backups,
                    what: match server.backup_count {
                        1 => "1 backup".into(),
                        count => format!("{count} backups"),
                    },
                },
            ]
        })
        // What is not worth a megabyte is not worth a line.
        .filter(|item| item.bytes >= 1024 * 1024)
        .collect();
    items.sort_by_key(|item| std::cmp::Reverse(item.bytes));
    items
}

/// One cell of a bar.
fn full() -> String {
    ui::glyph("█", "#")
}

/// How many of `width` cells `part` of `whole` fills: at least one for anything worth a
/// megabyte.
fn cells(part: u64, whole: u64, width: usize) -> usize {
    if part < 1024 * 1024 || whole == 0 {
        return 0;
    }
    let cells = (part as f64 / whole as f64 * width as f64).round() as usize;
    cells.clamp(1, width)
}

/// The whole disk as one bar, each part its own shade: what Core uses, what everything else
/// uses, and what is free.
fn disk_bar(core: u64, used: u64, total: u64) -> String {
    let core_cells = cells(core.min(used), total, BAR);
    let used_cells = cells(used, total, BAR).max(core_cells);
    let other = ui::glyph("▓", "=");
    let free = ui::glyph("░", ".");
    format!(
        "{}{}{}\n{}",
        ui::cyan(full().repeat(core_cells)),
        other.repeat(used_cells - core_cells),
        ui::dim(free.repeat(BAR - used_cells)),
        ui::dim(format!(
            "{} Core   {other} everything else   {free} free",
            ui::cyan(full())
        ))
    )
}

#[cfg(test)]
mod tests {
    use contracts::core::ServerUsage;

    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn the_biggest_items_come_first_and_empty_ones_are_left_out() {
        let usage = DiskUsage {
            total: 100 * GB,
            free: 40 * GB,
            servers: vec![
                ServerUsage {
                    id: "a".into(),
                    name: "Survival".into(),
                    files: 3 * GB,
                    backups: 9 * GB,
                    backup_count: 4,
                },
                ServerUsage {
                    id: "b".into(),
                    name: "Creative".into(),
                    files: 5 * GB,
                    backups: 0,
                    backup_count: 0,
                },
            ],
            shared: GB,
            game_files: 2 * GB,
        };
        let named: Vec<(String, String)> = biggest(&usage)
            .into_iter()
            .map(|item| (item.name, item.what))
            .collect();
        assert_eq!(
            named,
            [
                ("Survival".to_owned(), "4 backups".to_owned()),
                ("Creative".to_owned(), "server".to_owned()),
                ("Survival".to_owned(), "server".to_owned()),
            ]
        );
        assert_eq!(parts(&usage)[1], ("Backups", 9 * GB));
    }

    #[test]
    fn a_size_is_never_rounded_up_to_a_megabyte() {
        assert_eq!(size(0), "nothing");
        assert_eq!(size(165), "under 1 MB");
        assert_eq!(size(74 * 1024 * 1024), "74 MB");
        assert_eq!(size(1024 * GB), "1.0 TB");
    }

    #[test]
    fn a_bar_shows_anything_at_all_and_never_overflows() {
        const MB: u64 = 1024 * 1024;
        assert_eq!(cells(0, 100 * MB, 10), 0);
        assert_eq!(cells(165, 100 * MB, 10), 0);
        assert_eq!(cells(MB, 1000 * MB, 10), 1);
        assert_eq!(cells(50 * MB, 100 * MB, 10), 5);
        assert_eq!(cells(200 * MB, 100 * MB, 10), 10);
    }
}
