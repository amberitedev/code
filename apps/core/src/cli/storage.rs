//! `copal storage`: this Core's storage for shared files, and whether it offers it to other
//! people's shares or runs privately.

use color_eyre::eyre::Result;
use contracts::core::{
    ChangeStorage, GetStorage, StorageSettings, StorageStatus,
};

use super::{Client, Output, call::call};

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, clap::Args)]
pub struct StorageArgs {
    /// Offer storage to other people's shares.
    #[arg(long, conflicts_with = "disable")]
    enable: bool,
    /// Run privately: keep only your own shares here and use no one else's storage.
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
    let status = if change.enabled.is_none()
        && change.cap.is_none()
        && change.egress_cap.is_none()
    {
        call::<GetStorage>(client, &(), &(), ()).await?
    } else {
        call::<ChangeStorage>(client, &(), &(), change).await?
    };
    Ok(Output::new(describe(&status), &status))
}

fn describe(status: &StorageStatus) -> String {
    let gib = |bytes: u64| format!("{:.1} GiB", bytes as f64 / GIB as f64);
    let offered = if status.enabled {
        "Storage: offered to others"
    } else {
        "Storage: private, your own shares only"
    };
    [
        offered.to_owned(),
        format!(
            "Holding {} versions in {} of {} ({} free on this disk)",
            status.versions,
            gib(status.used),
            gib(status.cap),
            gib(status.free)
        ),
        format!(
            "Sent today: {} of {}",
            gib(status.egress_today),
            gib(status.egress_cap)
        ),
    ]
    .join("\n")
}
