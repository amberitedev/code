use color_eyre::eyre::{bail, Result};

use super::{
    admin::{self, Request, Response},
    require_confirmation, AccessCommand, InstanceCommand, OutputFormat,
};

pub(crate) async fn menu() -> Result<()> {
    let choice = inquire::Select::new(
        "Copal",
        vec![
            "Core status",
            "Service controls",
            "Instance maintenance",
            "Check for updates",
            "Diagnostics",
            "Exit",
        ],
    )
    .prompt()?;
    match choice {
        "Core status" => super::service::status(OutputFormat::Table).await,
        "Service controls" => super::service::menu().await,
        "Instance maintenance" => instance_menu().await,
        "Check for updates" => {
            println!("Use `copal update --check` or `copal update`.");
            Ok(())
        }
        "Diagnostics" => super::service::diagnose(OutputFormat::Table).await,
        _ => Ok(()),
    }
}

pub(crate) async fn instance(
    command: InstanceCommand,
    format: OutputFormat,
) -> Result<()> {
    match command {
        InstanceCommand::List => list_instances(format).await,
        InstanceCommand::Status { id } => instance_status(&id, format).await,
        InstanceCommand::Start { id, select, yes } => {
            operate_instances(id, select, yes, "start", |id| {
                Request::StartInstance { id }
            })
            .await
        }
        InstanceCommand::Stop { id, select, yes } => {
            operate_instances(id, select, yes, "stop", |id| {
                Request::StopInstance { id }
            })
            .await
        }
        InstanceCommand::Restart { id, select, yes } => {
            operate_instances(id, select, yes, "restart", |id| {
                Request::RestartInstance { id }
            })
            .await
        }
        InstanceCommand::Kill { id, yes } => {
            require_confirmation(
                yes,
                &format!("Force-kill {id}? Unsaved server data may be lost."),
            )?;
            admin_request(Request::KillInstance { id }).await?;
            println!("Force-kill requested.");
            Ok(())
        }
        InstanceCommand::Repair { id, yes } => {
            require_confirmation(
                yes,
                &format!("Repair server files for {id}?"),
            )?;
            admin_request(Request::RepairInstance { id }).await?;
            println!("Repair requested.");
            Ok(())
        }
        InstanceCommand::Console { id, send } => {
            if let Some(command) = send {
                admin_request(Request::SendConsoleCommand { id, command })
                    .await?;
                println!("Console command sent.");
                Ok(())
            } else {
                let data_dir = data_dir()?;
                println!("Attached to the console. Press Ctrl+C to detach.");
                admin::attach_console(&data_dir, id).await
            }
        }
    }
}

pub(crate) async fn access(
    command: AccessCommand,
    format: OutputFormat,
) -> Result<()> {
    match command {
        AccessCommand::Status => {
            println!("Use `copal check` to inspect local pairing state.");
            Ok(())
        }
        AccessCommand::List => list_members(format).await,
        AccessCommand::Roles => list_roles(format).await,
        AccessCommand::Invitations => list_invitations(format).await,
        AccessCommand::ReviewInvitation {
            id,
            approve,
            reject,
            yes,
        } => {
            if approve == reject {
                bail!("Specify exactly one of --approve or --reject.")
            }
            let action = if approve { "approve" } else { "reject" };
            require_confirmation(yes, &format!("{action} invitation {id}?"))?;
            admin_request(Request::ReviewInvitation {
                id,
                accept: approve,
            })
            .await?;
            println!(
                "Invitation {}.",
                if approve { "approved" } else { "rejected" }
            );
            Ok(())
        }
        AccessCommand::Remove { user_id, yes } => {
            require_confirmation(
                yes,
                &format!("Remove {user_id} from the Core group?"),
            )?;
            admin_request(Request::RemoveMember { user_id }).await?;
            println!("Member removed.");
            Ok(())
        }
        AccessCommand::ResetPairing { yes } => {
            require_confirmation(yes, "Reset pairing data?")?;
            super::reset_pairing().await
        }
    }
}

async fn list_members(format: OutputFormat) -> Result<()> {
    let Response::Members { members } =
        admin_request(Request::ListMembers).await?
    else {
        bail!("Unexpected response from local Core")
    };
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string(&members)?);
    } else if members.is_empty() {
        println!("No Core members found.");
    } else {
        println!("USER ID\tROLE\tSTATUS\tNAME");
        for member in members {
            println!(
                "{}\t{}\t{}\t{}",
                member.user_id,
                member.role,
                member.status,
                member.display_name.unwrap_or_default()
            );
        }
    }
    Ok(())
}

async fn list_roles(format: OutputFormat) -> Result<()> {
    let Response::Roles { roles } = admin_request(Request::ListRoles).await?
    else {
        bail!("Unexpected response from local Core")
    };
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string(&roles)?);
    } else if roles.is_empty() {
        println!("No roles found.");
    } else {
        println!("ID\tNAME\tRETIRED\tDESCRIPTION");
        for role in roles {
            println!(
                "{}\t{}\t{}\t{}",
                role.id,
                role.name,
                role.retired_at.unwrap_or_else(|| "no".to_owned()),
                role.description
            );
        }
    }
    Ok(())
}

async fn list_invitations(format: OutputFormat) -> Result<()> {
    let Response::Invitations { invitations } =
        admin_request(Request::ListInvitations).await?
    else {
        bail!("Unexpected response from local Core")
    };
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string(&invitations)?);
    } else if invitations.is_empty() {
        println!("No invitations found.");
    } else {
        println!("ID\tINVITEE\tSTATUS\tEXPIRES");
        for invitation in invitations {
            println!(
                "{}\t{}\t{}\t{}",
                invitation.id,
                invitation
                    .invitee_display_name
                    .unwrap_or(invitation.invitee_user_id),
                invitation.status,
                invitation.expires_at
            );
        }
    }
    Ok(())
}

async fn instance_menu() -> Result<()> {
    let choice = inquire::Select::new(
        "Instance maintenance",
        vec![
            "List instances",
            "Start selected",
            "Stop selected",
            "Restart selected",
            "Back",
        ],
    )
    .prompt()?;
    match choice {
        "List instances" => list_instances(OutputFormat::Table).await,
        "Start selected" => {
            operate_instances(None, true, false, "start", |id| {
                Request::StartInstance { id }
            })
            .await
        }
        "Stop selected" => {
            operate_instances(None, true, false, "stop", |id| {
                Request::StopInstance { id }
            })
            .await
        }
        "Restart selected" => {
            operate_instances(None, true, false, "restart", |id| {
                Request::RestartInstance { id }
            })
            .await
        }
        _ => Ok(()),
    }
}

async fn list_instances(format: OutputFormat) -> Result<()> {
    let Response::Instances { instances } =
        admin_request(Request::ListInstances).await?
    else {
        bail!("Unexpected response from local Core")
    };
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string(&instances)?);
    } else if instances.is_empty() {
        println!("No instances found.");
    } else {
        println!("ID\tSTATUS\tNAME");
        for instance in instances {
            println!("{}\t{}\t{}", instance.id, instance.status, instance.name);
        }
    }
    Ok(())
}

async fn instance_status(id: &str, format: OutputFormat) -> Result<()> {
    let Response::Instances { instances } =
        admin_request(Request::ListInstances).await?
    else {
        bail!("Unexpected response from local Core")
    };
    let instance = instances
        .into_iter()
        .find(|instance| instance.id.to_string() == id)
        .ok_or_else(|| color_eyre::eyre::eyre!("Instance not found: {id}"))?;
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string(&instance)?);
    } else {
        println!(
            "{}\nStatus: {}\nInstall: {}",
            instance.name, instance.status, instance.install_status
        );
    }
    Ok(())
}

async fn operate_instances(
    id: Option<String>,
    select: bool,
    yes: bool,
    action: &str,
    request: fn(String) -> Request,
) -> Result<()> {
    let ids = if select {
        select_instances().await?
    } else {
        vec![id.ok_or_else(|| {
            color_eyre::eyre::eyre!(
                "An instance ID is required unless --select is used."
            )
        })?]
    };
    if ids.is_empty() {
        println!("No instances selected.");
        return Ok(());
    }
    require_confirmation(
        yes,
        &format!("Request {action} for {} instance(s)?", ids.len()),
    )?;
    for id in ids {
        admin_request(request(id)).await?;
    }
    println!("{action} requested.");
    Ok(())
}

async fn select_instances() -> Result<Vec<String>> {
    let Response::Instances { instances } =
        admin_request(Request::ListInstances).await?
    else {
        bail!("Unexpected response from local Core")
    };
    let choices: Vec<_> = instances
        .iter()
        .map(|instance| {
            format!("{} [{}] ({})", instance.name, instance.id, instance.status)
        })
        .collect();
    let selected =
        inquire::MultiSelect::new("Select instances", choices).prompt()?;
    let ids = selected
        .into_iter()
        .filter_map(|choice| {
            choice
                .split('[')
                .nth(1)?
                .split(']')
                .next()
                .map(str::to_owned)
        })
        .collect();
    Ok(ids)
}

async fn admin_request(request: Request) -> Result<Response> {
    let data_dir = data_dir()?;
    admin::request(&data_dir, request).await
}

fn data_dir() -> Result<std::path::PathBuf> {
    super::metadata::load()?
		.and_then(|metadata| metadata.data_dir)
		.ok_or_else(|| color_eyre::eyre::eyre!("Copal installation metadata has no data directory. Re-run `copal install --env-file …`."))
}
