//! `copal storage`: the storage this Core offers for other people's shared files.

use axum::http::Method;
use color_eyre::eyre::Result;

use super::{Client, Output, on_off};
use crate::api::storage::{StorageSettings, StorageStatus};

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, clap::Args)]
pub struct StorageArgs {
    /// Offer storage.
    #[arg(long, conflicts_with = "disable")]
    enable: bool,
    /// Stop offering storage. Your own sharing is not affected.
    #[arg(long)]
    disable: bool,
    /// The most to hold, in GiB.
    #[arg(long, value_name = "GIB")]
    cap: Option<u64>,
    /// The most to send a day, in GiB.
    #[arg(long, value_name = "GIB")]
    egress: Option<u64>,
}

pub async fn run(client: &Client, args: StorageArgs) -> Result<Output> {
    let gib = |value: u64| {
        i64::try_from(value.saturating_mul(GIB)).unwrap_or(i64::MAX)
    };
    let change = StorageSettings {
        enabled: (args.enable || args.disable).then_some(args.enable),
        cap: args.cap.map(gib),
        egress_cap: args.egress.map(gib),
    };
    let status: StorageStatus = if change.enabled.is_none()
        && change.cap.is_none()
        && change.egress_cap.is_none()
    {
        client.get("/storage").await?
    } else {
        client
            .call(Method::PATCH, "/storage", Some(&change))
            .await?
    };
    Ok(Output::new(describe(&status), &status))
}

fn describe(status: &StorageStatus) -> String {
    let gib = |bytes: u64| format!("{:.1} GiB", bytes as f64 / GIB as f64);
    let mut lines = vec![format!(
        "Storage for shared files: {}",
        on_off(status.enabled)
    )];
    if status.enabled {
        lines.push(format!(
            "Holding {} versions in {} of {} ({} free on this disk)",
            status.versions,
            gib(status.used),
            gib(status.cap),
            gib(status.free)
        ));
        lines.push(format!(
            "Sent today: {} of {}",
            gib(status.egress_today),
            gib(status.egress_cap)
        ));
    }
    lines.join("\n")
}
