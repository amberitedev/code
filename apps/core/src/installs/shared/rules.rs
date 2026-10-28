//! Library rules and the host platform, for the server.
//!
//! Mirrors app-lib's `launcher::parse_rules` and `util::platform`, which `minecraft-meta-manifest`
//! does not carry. A server has no client features (demo, custom resolution, quick play), so a
//! feature rule never matches. OS rules are checked against the machine Core runs on, which is
//! where the server's Java runs.

use minecraft_meta_manifest::minecraft::{
    OperatingSystem as Os, OperatingSystemRule, Rule, RuleAction,
};

/// The base OS Core runs on, app-lib's `OperatingSystemExt::native`.
fn native() -> Os {
    match std::env::consts::OS {
        "windows" => Os::Windows,
        "macos" => Os::Osx,
        "linux" => Os::Linux,
        _ => Os::Unknown,
    }
}

/// The OS variant for a Java architecture, app-lib's `OperatingSystemExt::native_arch`.
pub fn native_arch(java_arch: &str) -> Os {
    let arm64 = java_arch == "aarch64";
    match std::env::consts::OS {
        "windows" if arm64 => Os::WindowsArm64,
        "windows" => Os::Windows,
        "linux" if arm64 => Os::LinuxArm64,
        "linux" if java_arch == "arm" => Os::LinuxArm32,
        "linux" => Os::Linux,
        "macos" if arm64 => Os::OsxArm64,
        "macos" => Os::Osx,
        _ => Os::Unknown,
    }
}

/// The base OS of a variant, app-lib's `OperatingSystemExt::get_os`.
fn base(os: Os) -> Os {
    match os {
        Os::OsxArm64 => Os::Osx,
        Os::LinuxArm32 | Os::LinuxArm64 => Os::Linux,
        Os::WindowsArm64 => Os::Windows,
        other => other,
    }
}

/// JVM classpath separator for the host platform.
pub fn classpath_separator(java_arch: &str) -> &'static str {
    match native_arch(java_arch) {
        Os::Windows | Os::WindowsArm64 => ";",
        _ => ":",
    }
}

fn os_rule(rule: &OperatingSystemRule, java_arch: &str) -> bool {
    let mut matches = true;
    if let Some(arch) = &rule.arch {
        matches &= !matches!(arch.as_str(), "x86" | "arm");
    }
    if let Some(name) = rule.name {
        // Servers run on a modern JVM, so only the post-1.18 matching applies: the base OS or
        // the exact variant.
        matches &= native() == base(name) || native_arch(java_arch) == name;
    }
    matches
}

fn parse_rule(rule: &Rule, java_arch: &str) -> Option<bool> {
    let matched = match rule {
        Rule { os: Some(os), .. } => os_rule(os, java_arch),
        Rule {
            features: Some(_), ..
        } => false,
        _ => return Some(true),
    };
    match rule.action {
        RuleAction::Allow => Some(matched),
        RuleAction::Disallow => matched.then_some(false),
    }
}

/// Whether a rule set allows a library: any `false` disallows, all-undecided disallows, a set of
/// only disallow rules allows what none of them match. app-lib's `parse_rules`.
pub fn parse_rules(rules: &[Rule], java_arch: &str) -> bool {
    let mut decided: Vec<Option<bool>> = rules
        .iter()
        .map(|rule| parse_rule(rule, java_arch))
        .collect();
    if rules
        .iter()
        .all(|rule| matches!(rule.action, RuleAction::Disallow))
    {
        decided.push(Some(true));
    }
    !(decided.contains(&Some(false)) || decided.iter().all(Option::is_none))
}
