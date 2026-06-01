//! Library/argument rule evaluation for the shared store.
//!
//! Port of app-lib's `launcher::parse_rules` + `util::platform`, specialised for
//! the server: there is no quick-play and none of the client feature flags
//! (demo, custom resolution, quick-play) are ever active, so feature rules
//! resolve as if all those features are disabled. OS rules are evaluated against
//! the host the Core runs on, which is exactly where the server JVM launches.

use daedalus::minecraft::{Os, OsRule, Rule, RuleAction};

/// JVM classpath separator for the host platform.
pub fn classpath_separator(java_arch: &str) -> &'static str {
	match Os::native_arch(java_arch) {
		Os::Osx
		| Os::OsxArm64
		| Os::Linux
		| Os::LinuxArm32
		| Os::LinuxArm64
		| Os::Unknown => ":",
		Os::Windows | Os::WindowsArm64 => ";",
	}
}

fn os_rule(rule: &OsRule, java_arch: &str) -> bool {
	let mut rule_match = true;

	if let Some(ref arch) = rule.arch {
		rule_match &= !matches!(arch.as_str(), "x86" | "arm");
	}

	if let Some(name) = &rule.name {
		// Servers always run on a modern, natively-supported JVM, so we use the
		// post-1.18 matching path (base OS or arch-specific variant).
		rule_match &= Os::native() == name.get_os()
			|| &Os::native_arch(java_arch) == name;
	}

	rule_match
}

fn parse_rule(rule: &Rule, java_arch: &str) -> Option<bool> {
	let res = match rule {
		Rule { os: Some(os), .. } => os_rule(os, java_arch),
		Rule {
			features: Some(_), ..
		} => {
			// No client features are active on a server.
			false
		}
		_ => return Some(true),
	};

	match rule.action {
		RuleAction::Allow => Some(res),
		RuleAction::Disallow => {
			if res {
				Some(false)
			} else {
				None
			}
		}
	}
}

/// Evaluate a rule set: all-none → disallowed, any false → disallowed, otherwise
/// allowed. Matches app-lib's `parse_rules` semantics.
pub fn parse_rules(rules: &[Rule], java_arch: &str) -> bool {
	let mut x = rules
		.iter()
		.map(|r| parse_rule(r, java_arch))
		.collect::<Vec<Option<bool>>>();

	if rules
		.iter()
		.all(|r| matches!(r.action, RuleAction::Disallow))
	{
		x.push(Some(true));
	}

	!(x.iter().any(|v| v == &Some(false)) || x.iter().all(|v| v.is_none()))
}
