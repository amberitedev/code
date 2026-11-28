//! `copal doctor`: everything Core needs to work, checked now. Problems come first, each with
//! its fix, and `--fix` applies the ones a program can apply.
//!
//! Core checks what only it can see (pairing, the backend, the router, Java) and answers over
//! the local socket. The service, the firewall and the disk are checked here, because they are
//! what is wrong when Core does not answer, and repairing the first two takes an admin.

use std::time::Duration;

use color_eyre::eyre::Result;
use contracts::{
    Empty,
    core::{CheckDetail, CheckState, Doctor, DoctorCheck, GetDoctor},
};
use serde::Serialize;

use super::{
    Client, Done,
    call::call,
    client::Unreachable,
    ui::{self, Mark, Ui},
};
use crate::{
    daemon::{
        config::{self, Config},
        install::{self, as_admin},
        service,
    },
    network::firewall,
};

/// The port a Core listens on unless its settings say otherwise.
const DEFAULT_PORT: u16 = 16662;
/// The checks Core answers, for naming them when it does not.
/// In the order their groups are shown.
const CORE_CHECKS: [&str; 5] =
    ["java", "pairing", "backend", "router", "ports"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Group {
    Computer,
    Amberite,
    Network,
}

impl Group {
    const ALL: [Self; 3] = [Self::Computer, Self::Amberite, Self::Network];

    fn title(self) -> &'static str {
        match self {
            Self::Computer => "This computer",
            Self::Amberite => "Amberite",
            Self::Network => "Network",
        }
    }

    fn of(check: &str) -> Self {
        match check {
            "pairing" | "backend" => Self::Amberite,
            "router" | "ports" => Self::Network,
            _ => Self::Computer,
        }
    }
}

/// Why Core's own checks are missing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Silent {
    /// Nothing answers on the local socket.
    Down,
    /// Core answers only the user who set it up, and this is root: `sudo copal doctor`, which
    /// is how the firewall is read and repaired on Linux.
    Root,
    /// Core answers only the user who set it up, and this is someone else.
    Foreign,
    /// Core answered, with an error, or with something this CLI cannot read.
    Other(String),
}

impl Silent {
    fn why(&self) -> String {
        match self {
            Self::Down => "They need Core running".into(),
            Self::Root | Self::Foreign => {
                "Core answers only the user who set it up".into()
            }
            Self::Other(why) => format!("Core did not answer: {why}"),
        }
    }
}

/// Something `--fix` can do. Both take an admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Repair {
    StartService,
    OpenFirewall(u16),
}

impl Repair {
    fn label(self) -> String {
        match self {
            Self::StartService => "Start Core's service".into(),
            Self::OpenFirewall(port) => format!(
                "Let Core's ports through this computer's firewall (TCP and UDP {port}, and the server ports)"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Check {
    #[serde(flatten)]
    check: DoctorCheck,
    group: Group,
    /// `copal doctor --fix` can repair it.
    fixable: bool,
    #[serde(skip)]
    repair: Option<Repair>,
    /// Skipped because Core did not answer. Said once for all of them, not once each.
    #[serde(skip)]
    unanswered: bool,
}

#[derive(Serialize)]
struct Report {
    /// In the order they are shown.
    checks: Vec<Check>,
    problems: usize,
    warnings: usize,
    ok: usize,
    skipped: usize,
    /// The checks `--fix` repaired.
    fixed: Vec<String>,
}

/// `copal doctor`. It fails, and exits non-zero, when a check failed; warnings do not.
pub async fn run(
    ui: &Ui,
    client: &Client,
    fix: bool,
    verbose: bool,
) -> Result<Done> {
    ui.intro("copal doctor");
    let admin = install::is_admin();
    let mut checks = examine(ui, client, admin).await;
    let mut fixed = Vec::new();
    let repairs = repairs(&checks);
    if fix && admin && !repairs.is_empty() {
        fixed = apply(ui, client, &repairs).await;
        checks = examine(ui, client, admin).await;
    }
    show(ui, &checks, verbose);
    if !fix && admin && !repairs.is_empty() && ui.terminal() {
        let items: Vec<(usize, String, String)> = repairs
            .iter()
            .enumerate()
            .map(|(index, (name, repair))| {
                (index, repair.label(), format!("fixes {name}"))
            })
            .collect();
        let chosen = ui.multiselect(
            "Fix these now? Space unticks one, Enter goes on.",
            &items,
            "Pass --fix to apply them",
        )?;
        let chosen: Vec<(String, Repair)> = chosen
            .into_iter()
            .map(|index| repairs[index].clone())
            .collect();
        if !chosen.is_empty() {
            fixed = apply(ui, client, &chosen).await;
            checks = examine(ui, client, admin).await;
            show(ui, &checks, verbose);
        }
    }

    let Tally {
        problems,
        warnings,
        ok,
        skipped,
        line,
        needs_admin,
    } = tally(&checks, fix, admin);
    ui.outro(line);
    let report = Report {
        checks,
        problems,
        warnings,
        ok,
        skipped,
        fixed,
    };
    Ok(if problems > 0 || needs_admin {
        Done::failed(&report)
    } else {
        Done::ok(&report)
    })
}

/// The counts, and the report's last line.
struct Tally {
    problems: usize,
    warnings: usize,
    ok: usize,
    skipped: usize,
    line: String,
    /// `--fix` without an admin repaired nothing, and that is why the run failed.
    needs_admin: bool,
}

fn tally(checks: &[Check], fix: bool, admin: bool) -> Tally {
    let count = |state| {
        checks
            .iter()
            .filter(|check| check.check.state == state)
            .count()
    };
    let (problems, warnings, ok, skipped) = (
        count(CheckState::Fail),
        count(CheckState::Warn),
        count(CheckState::Ok),
        count(CheckState::Skip),
    );
    // Worst first, the order the report itself goes in.
    let mut totals = Vec::new();
    if problems > 0 {
        totals.push(plural(problems, "problem"));
    }
    if warnings > 0 {
        totals.push(plural(warnings, "warning"));
    }
    totals.push(format!("{ok} ok"));
    if skipped > 0 {
        totals.push(format!("{skipped} skipped"));
    }
    let totals = ui::sentence(&totals.join(", "));
    let left: Vec<String> =
        repairs(checks).into_iter().map(|(name, _)| name).collect();
    let left = left.join(" and ");
    let needs_admin = fix && !admin && !left.is_empty();
    // A check skipped for want of rights carries how to have it made.
    let unchecked = checks.iter().find_map(|check| {
        (check.check.state == CheckState::Skip)
            .then(|| check.check.fix.first())
            .flatten()
    });
    // A skip is expected where something is switched off: forwarding, or the backend. One on
    // this computer, or one Core never answered, is no clean bill.
    let all_checked = checks.iter().all(|check| {
        check.check.state != CheckState::Skip
            || (!check.unanswered && check.group != Group::Computer)
    });
    let line = if needs_admin {
        // How is under the problem itself, where the fix always is.
        format!(
            "{totals}. Nothing was fixed: fixing the {left} needs an admin."
        )
    } else if !left.is_empty() {
        format!(
            "{totals}. {} the {left} with: copal doctor --fix",
            if admin { "Fix" } else { "As an admin, fix" }
        )
    } else if let Some(step) = unchecked {
        format!("{totals}. {step}")
    } else if problems + warnings == 0 && all_checked {
        format!("{totals}. Everything works.")
    } else {
        format!("{totals}.")
    };
    Tally {
        problems,
        warnings,
        ok,
        skipped,
        line,
        needs_admin,
    }
}

fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

/// Run every check, showing a bar in a terminal while they run.
async fn examine(ui: &Ui, client: &Client, admin: bool) -> Vec<Check> {
    let progress =
        ui.progress(CORE_CHECKS.len() as u64 + 3, "Checking the service");
    let service = service::state();
    progress.step();
    progress.message("Asking Core, the router and Amberite");
    let core = call::<GetDoctor>(client, &(), &(), Empty).await;
    for _ in CORE_CHECKS {
        progress.step();
    }
    progress.message("Checking the firewall and the disk");
    let port = core
        .as_ref()
        .map(|doctor| doctor.port)
        .ok()
        .or_else(|| Config::load().ok().map(|config| config.port))
        .unwrap_or(DEFAULT_PORT);
    let rules = firewall::check(port);
    progress.step();
    // The data folder's disk, or the one it will be made on where Core was never set up.
    let folder = config::data_dir();
    let disk = crate::api::doctor::disk_check(
        folder
            .ancestors()
            .find(|folder| folder.exists())
            .unwrap_or(&folder),
    );
    progress.step();
    let core = core.map_err(|error| {
        if error.is::<Unreachable>() {
            Silent::Down
        // Only on Linux and macOS does Core take a connection to refuse it; on Windows the
        // pipe itself keeps other users out, and a closed connection is a Core that died.
        } else if cfg!(unix) && error.is::<super::client::Closed>() {
            if admin { Silent::Root } else { Silent::Foreign }
        } else {
            Silent::Other(format!("{error:#}"))
        }
    });
    progress.clear();
    checks(service, core, rules, disk, port, admin)
}

/// Every check in the order it is shown, from what was found.
fn checks(
    service: service::State,
    core: Result<Doctor, Silent>,
    rules: firewall::Rules,
    disk: DoctorCheck,
    port: u16,
    admin: bool,
) -> Vec<Check> {
    // Named for what it does, so `fix: Open it with` reads as one thing in plain output.
    let fix_with = |verb: &str| {
        if admin {
            format!("{verb} it with: copal doctor --fix")
        } else {
            as_admin(&format!("{verb} it with"), "copal doctor --fix")
        }
    };
    let wrap = |check: DoctorCheck, repair: Option<Repair>| Check {
        group: Group::of(&check.name),
        check,
        fixable: repair.is_some(),
        repair,
        unanswered: false,
    };
    let own = |name: &str,
               state,
               summary: String,
               fix: Vec<String>,
               repair: Option<Repair>| {
        wrap(
            DoctorCheck {
                name: name.into(),
                state,
                summary,
                details: Vec::new(),
                fix,
            },
            repair,
        )
    };
    let set_up = as_admin("Set it up with", "copal setup");
    let service_check = match (&core, service) {
        (Ok(doctor), service::State::Running) if doctor.installed => own(
            "service",
            CheckState::Ok,
            "running, and starts with this computer".into(),
            vec![],
            None,
        ),
        (Ok(doctor), _) if doctor.installed => own(
            "service",
            CheckState::Warn,
            "Core runs, but not from its service, so it will not start with this computer".into(),
            vec![as_admin(
                "Stop the Core that runs by hand, then start its service with",
                if cfg!(windows) {
                    "sc start copal"
                } else if cfg!(target_os = "macos") {
                    "launchctl kickstart system/dev.amberite.copal"
                } else {
                    "systemctl start copal"
                },
            )],
            None,
        ),
        (Ok(_), _) => own(
            "service",
            CheckState::Warn,
            "Core runs by hand, not as a service, so it stops when its terminal closes".into(),
            vec![set_up],
            None,
        ),
        (Err(_), service::State::NotRegistered) => own(
            "service",
            CheckState::Fail,
            "Core is not set up on this computer yet".into(),
            vec![set_up],
            None,
        ),
        (Err(_), service::State::Stopped) => own(
            "service",
            CheckState::Fail,
            "Core's service is stopped".into(),
            vec![fix_with("Start")],
            Some(Repair::StartService),
        ),
        // Root cannot ask Core, but the service manager says it runs.
        (Err(Silent::Root), service::State::Running) => own(
            "service",
            CheckState::Ok,
            "the service is running".into(),
            vec![],
            None,
        ),
        (Err(Silent::Other(why)), service::State::Running) => own(
            "service",
            CheckState::Fail,
            format!("Core answered with an error: {why}"),
            vec!["See why with: copal logs".into()],
            None,
        ),
        (Err(_), service::State::Running) => own(
            "service",
            CheckState::Fail,
            "the service runs, but Core does not answer this user".into(),
            vec![
                "Use the user account that set Core up.".into(),
                as_admin("Read Core's log with", "copal logs"),
            ],
            None,
        ),
    };
    let ports = format!(
        "TCP {port}, UDP {port} and the server ports {}-{}",
        firewall::SERVER_PORTS.start(),
        firewall::SERVER_PORTS.end()
    );
    // Setup is what opens the firewall: where it never ran, that is the one thing to fix.
    let never_set_up = match &core {
        Ok(doctor) => !doctor.installed,
        Err(_) => service == service::State::NotRegistered,
    };
    let mut firewall_check = match rules {
        _ if never_set_up => own(
            "firewall",
            CheckState::Skip,
            "setting Core up opens it".into(),
            vec![],
            None,
        ),
        firewall::Rules::Open => own(
            "firewall",
            CheckState::Ok,
            "lets Core and its servers in".into(),
            vec![],
            None,
        ),
        firewall::Rules::Missing { found } => own(
            "firewall",
            CheckState::Fail,
            found,
            vec![fix_with("Open")],
            Some(Repair::OpenFirewall(port)),
        ),
        // Not a problem, and not a clean bill either: its fix is how to have it checked.
        firewall::Rules::Unknown { why } => own(
            "firewall",
            CheckState::Skip,
            why,
            // More rights can read it; an admin who could not has nothing further to try.
            if admin {
                vec![]
            } else {
                vec![as_admin("Check the firewall with", "copal doctor")]
            },
            None,
        ),
    };
    firewall_check.check.details = vec![CheckDetail {
        label: "Core needs".into(),
        value: ports,
    }];
    let mut checks = vec![service_check, firewall_check, wrap(disk, None)];
    match core {
        Ok(doctor) => checks
            .extend(doctor.checks.into_iter().map(|check| wrap(check, None))),
        Err(silent) => {
            // Root is whom the firewall is read as, and whom Core does not answer: the rest
            // is one more run away.
            let fix = match silent {
                Silent::Root => {
                    vec![
                        "Check the rest, without sudo, with: copal doctor"
                            .to_owned(),
                    ]
                }
                Silent::Down | Silent::Foreign | Silent::Other(_) => vec![],
            };
            checks.extend(CORE_CHECKS.iter().map(|name| Check {
                unanswered: true,
                ..own(name, CheckState::Skip, silent.why(), fix.clone(), None)
            }));
        }
    }
    // Stable, so a group keeps the order its checks came in.
    checks.sort_by_key(|check| {
        Group::ALL.iter().position(|group| *group == check.group)
    });
    checks
}

/// The repairs the checks call for, each with the check it fixes.
fn repairs(checks: &[Check]) -> Vec<(String, Repair)> {
    checks
        .iter()
        .filter_map(|check| Some((check.check.name.clone(), check.repair?)))
        .collect()
}

/// Apply `repairs`, saying how each went, and return the checks that were repaired.
async fn apply(
    ui: &Ui,
    client: &Client,
    repairs: &[(String, Repair)],
) -> Vec<String> {
    let mut fixed = Vec::new();
    for (name, repair) in repairs {
        let spinner = ui.spinner(repair.label());
        let result = match *repair {
            Repair::StartService => match service::start() {
                Ok(()) => started(client).await,
                Err(error) => Err(format!("{error:#}")),
            },
            Repair::OpenFirewall(port) => firewall::open(port)
                .map(|_| ())
                .map_err(|error| format!("{error:#}")),
        };
        match result {
            Ok(()) => {
                spinner.done(format!("Fixed {name}"));
                fixed.push(name.clone());
            }
            Err(error) => {
                spinner.failed(format!("Could not fix {name}: {error}"))
            }
        }
    }
    fixed
}

/// Wait for a Core that was just started to answer. On Linux and macOS this runs as root,
/// whom Core hears and then refuses: that too says it is up and listening.
async fn started(client: &Client) -> Result<(), String> {
    for _ in 0..30 {
        match call::<contracts::core::CheckHealth>(client, &(), &(), Empty)
            .await
        {
            Ok(_) => return Ok(()),
            Err(error) if cfg!(unix) && error.is::<super::client::Closed>() => {
                return Ok(());
            }
            Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
    Err(
        "the service started, but Core does not answer yet. See why with: copal logs"
            .into(),
    )
}

/// The report for a terminal: what is wrong first, then every check under its group.
fn show(ui: &Ui, checks: &[Check], verbose: bool) {
    if !ui.terminal() {
        ui.say(Mark::Done, plain(checks, verbose));
        return;
    }
    let problems = problems(checks, true);
    if !problems.is_empty() {
        let failed = checks
            .iter()
            .any(|check| check.check.state == CheckState::Fail);
        ui.say(if failed { Mark::Problem } else { Mark::Warning }, problems);
    }
    for group in Group::ALL {
        let rows = rows(checks, group, verbose, true, false);
        if !rows.is_empty() {
            ui.say(Mark::Done, format!("{}\n{rows}", group.title()));
        }
    }
    if let Some(unanswered) = unanswered(checks) {
        ui.say(Mark::Info, ui::dim(unanswered));
    }
}

/// The checks a Core that did not answer left out, and why, in one line for all of them.
fn unanswered(checks: &[Check]) -> Option<String> {
    let why = &checks.iter().find(|check| check.unanswered)?.check.summary;
    Some(format!("Skipped: {}. {why}.", CORE_CHECKS.join(", ")))
}

/// The whole report as plain lines, each check once: what is wrong, then the rest by group.
fn plain(checks: &[Check], verbose: bool) -> String {
    let mut blocks = Vec::new();
    let problems = problems(checks, false);
    if !problems.is_empty() {
        blocks.push(problems);
    }
    for group in Group::ALL {
        let rows = rows(checks, group, verbose, false, true);
        if !rows.is_empty() {
            blocks.push(format!("{}:\n{rows}", group.title()));
        }
    }
    blocks.extend(unanswered(checks));
    blocks.join("\n")
}

fn wrong(check: &Check) -> bool {
    matches!(check.check.state, CheckState::Fail | CheckState::Warn)
}

/// What is wrong, worst first, each with the facts behind it and its fix. Empty when nothing
/// is.
fn problems(checks: &[Check], drawn: bool) -> String {
    let of = |state| {
        checks
            .iter()
            .filter(move |check| check.check.state == state)
    };
    let found: Vec<&Check> =
        of(CheckState::Fail).chain(of(CheckState::Warn)).collect();
    if found.is_empty() {
        return String::new();
    }
    let failed = of(CheckState::Fail).count();
    let heading = match (failed, found.len() - failed) {
        (0, warnings) => plural(warnings, "warning"),
        (failed, 0) => plural(failed, "problem"),
        (failed, warnings) => format!(
            "{} and {}",
            plural(failed, "problem"),
            plural(warnings, "warning")
        ),
    };
    let heading = if drawn {
        ui::bold(heading)
    } else {
        format!("{heading}:")
    };
    let each: Vec<String> = found
        .into_iter()
        .map(|check| {
            let mut lines = vec![row(&check.check, drawn)];
            lines.extend(facts(&check.check, drawn));
            let arrow = if drawn {
                ui::glyph("→", "->")
            } else {
                "fix:".to_owned()
            };
            lines.extend(
                check
                    .check
                    .fix
                    .iter()
                    .map(|step| format!("    {arrow} {step}")),
            );
            lines.join("\n")
        })
        .collect();
    // In a terminal each one stands apart; plain lines stay one to a line.
    let gap = if drawn { "\n\n" } else { "\n" };
    format!("{heading}\n{}", each.join(gap))
}

/// The rows of one group. The facts behind a check that is not fine are with the problems; with
/// `verbose` every check shows its own here. `fine_only` leaves out the checks the problems
/// already listed.
fn rows(
    checks: &[Check],
    group: Group,
    verbose: bool,
    drawn: bool,
    fine_only: bool,
) -> String {
    let mut lines = Vec::new();
    for check in checks.iter().filter(|check| {
        check.group == group
            && !check.unanswered
            && !(fine_only && wrong(check))
    }) {
        lines.push(row(&check.check, drawn));
        if verbose {
            lines.extend(facts(&check.check, drawn));
        }
    }
    lines.join("\n")
}

/// The facts behind a check, one to a line.
fn facts(check: &DoctorCheck, drawn: bool) -> Vec<String> {
    let width = check
        .details
        .iter()
        .map(|detail| detail.label.chars().count())
        .max()
        .unwrap_or(0);
    check
        .details
        .iter()
        .map(|CheckDetail { label, value }| {
            let line = format!("    {label:<width$}  {value}");
            if drawn { ui::dim(line) } else { line }
        })
        .collect()
}

/// `✔ service   running`, or `ok   service: running` as plain text.
fn row(check: &DoctorCheck, drawn: bool) -> String {
    let DoctorCheck { name, summary, .. } = check;
    if drawn {
        let mark = match check.state {
            CheckState::Ok => ui::green(ui::glyph("✔", "ok")),
            CheckState::Warn => ui::yellow(ui::glyph("▲", "!")),
            CheckState::Fail => ui::red(ui::glyph("✖", "x")),
            CheckState::Skip => ui::dim(ui::glyph("–", "-")),
        };
        let summary = match check.state {
            CheckState::Skip => ui::dim(format!("skipped: {summary}")),
            _ => summary.clone(),
        };
        format!("{mark} {name:<9} {summary}")
    } else {
        let mark = match check.state {
            CheckState::Ok => "ok",
            CheckState::Warn => "warn",
            CheckState::Fail => "fail",
            CheckState::Skip => "skip",
        };
        format!("{mark:<4} {name}: {summary}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core(checks: Vec<DoctorCheck>) -> Doctor {
        Doctor {
            port: 16662,
            installed: true,
            checks,
        }
    }

    fn check(name: &str, state: CheckState, summary: &str) -> DoctorCheck {
        DoctorCheck {
            name: name.into(),
            state,
            summary: summary.into(),
            details: vec![],
            fix: vec![],
        }
    }

    fn disk() -> DoctorCheck {
        check("disk", CheckState::Ok, "plenty")
    }

    /// The case doctor exists for: a Core set up before Connect has no UDP rule.
    #[test]
    fn a_missing_firewall_rule_is_a_problem_that_fix_repairs() {
        let checks = checks(
            service::State::Running,
            Ok(core(vec![check("pairing", CheckState::Ok, "paired")])),
            firewall::Rules::Missing {
                found: "the rule allows TCP but not UDP 16662".into(),
            },
            disk(),
            16662,
            true,
        );
        assert_eq!(
            repairs(&checks),
            [("firewall".to_owned(), Repair::OpenFirewall(16662))]
        );
        // Every check is there once: what is wrong first, then the rest.
        assert_eq!(
            plain(&checks, false),
            "1 problem:\n\
             fail firewall: the rule allows TCP but not UDP 16662\n    \
             Core needs  TCP 16662, UDP 16662 and the server ports 25565-25664\n    \
             fix: Open it with: copal doctor --fix\n\
             This computer:\n\
             ok   service: running, and starts with this computer\n\
             ok   disk: plenty\n\
             Amberite:\n\
             ok   pairing: paired"
        );
    }

    #[test]
    fn a_stopped_core_is_one_problem_and_what_it_left_out_is_said_once() {
        let checks = checks(
            service::State::Stopped,
            Err(Silent::Down),
            firewall::Rules::Open,
            disk(),
            16662,
            false,
        );
        assert_eq!(
            repairs(&checks),
            [("service".to_owned(), Repair::StartService)]
        );
        let skipped = checks
            .iter()
            .filter(|check| check.check.state == CheckState::Skip)
            .count();
        assert_eq!(skipped, CORE_CHECKS.len());
        assert!(plain(&checks, false).ends_with(
            "ok   disk: plenty\n\
             Skipped: java, pairing, backend, router, ports. They need Core running."
        ));
    }

    /// `sudo copal doctor` is how the firewall is read on Linux, and Core does not answer
    /// root: that is not the service failing.
    #[test]
    fn under_sudo_a_running_service_is_fine_and_the_firewall_is_what_gets_checked()
     {
        let checks = checks(
            service::State::Running,
            Err(Silent::Root),
            firewall::Rules::Missing {
                found: "ufw allows TCP but not UDP 16662".into(),
            },
            disk(),
            16662,
            true,
        );
        assert_eq!(checks[0].check.state, CheckState::Ok);
        assert_eq!(
            repairs(&checks),
            [("firewall".to_owned(), Repair::OpenFirewall(16662))]
        );
        assert!(
            plain(&checks, false)
                .ends_with("Core answers only the user who set it up.")
        );
    }

    /// Another user than the one who set Core up is told so, not that Core has an error.
    #[test]
    fn a_user_core_does_not_answer_is_told_to_use_the_one_it_does() {
        let checks = checks(
            service::State::Running,
            Err(Silent::Foreign),
            firewall::Rules::Open,
            disk(),
            16662,
            false,
        );
        assert_eq!(checks[0].check.state, CheckState::Fail);
        assert_eq!(
            checks[0].check.fix[0],
            "Use the user account that set Core up."
        );
    }

    #[test]
    fn the_last_line_never_calls_unchecked_things_fine() {
        let healthy = |core, rules| {
            checks(service::State::Running, core, rules, disk(), 16662, false)
        };
        let line = |checks: &[Check], fix| tally(checks, fix, false).line;
        // Everything answered, and what is switched off is expected to be skipped.
        let all = healthy(
            Ok(core(vec![
                check("pairing", CheckState::Ok, "paired"),
                check("router", CheckState::Skip, "forwarding is off"),
            ])),
            firewall::Rules::Open,
        );
        assert_eq!(line(&all, false), "4 ok, 1 skipped. Everything works.");
        // Under sudo Core answers nothing: the rest is one more run away.
        let root = healthy(Err(Silent::Root), firewall::Rules::Open);
        assert_eq!(
            line(&root, false),
            "3 ok, 5 skipped. Check the rest, without sudo, with: copal doctor"
        );
        // A firewall nobody could read is not fine either.
        let unread = healthy(
            Ok(core(vec![])),
            firewall::Rules::Unknown {
                why: "only root can read the firewall".into(),
            },
        );
        assert!(
            line(&unread, false)
                .to_lowercase()
                .contains("check the firewall with"),
            "{}",
            line(&unread, false)
        );
        // What `--fix` can repair is named, and without an admin it says why it did not.
        let broken = healthy(
            Ok(core(vec![])),
            firewall::Rules::Missing {
                found: "no UDP rule".into(),
            },
        );
        assert!(
            line(&broken, false)
                .ends_with("fix the firewall with: copal doctor --fix")
        );
        let refused = tally(&broken, true, false);
        assert!(refused.needs_admin);
        assert!(refused.line.ends_with("needs an admin."));
    }

    #[test]
    fn a_core_run_by_hand_is_a_warning_with_no_repair() {
        let mut doctor = core(vec![]);
        doctor.installed = false;
        let checks = checks(
            service::State::NotRegistered,
            Ok(doctor),
            // Whatever the firewall says: setup, which opens it, never ran.
            firewall::Rules::Missing {
                found: "no rule".into(),
            },
            disk(),
            16662,
            true,
        );
        assert_eq!(checks[0].check.state, CheckState::Warn);
        assert_eq!(checks[1].check.state, CheckState::Skip);
        assert!(repairs(&checks).is_empty());
    }
}
