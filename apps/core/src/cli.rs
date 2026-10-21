//! The CLI commands that talk to a running Core over the local socket: `status` and `servers`.
//! Just enough to control servers by hand; the full command set is step 7.

use std::time::Duration;

use axum::http::{Method, Request, header};
use bytes::Bytes;
use clap::Subcommand;
use color_eyre::eyre::{Result, WrapErr, bail};
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    access::local,
    api::{
        error::ApiError,
        servers::{
            CreateServerRequest, CreateServerResponse, PowerAction,
            PowerRequest, ReinstallRequest, Server, ServerGetResponse,
            ServerPowerState,
        },
    },
    daemon::{config, paths::Paths},
};

#[derive(Subcommand)]
pub enum ServersCommand {
    /// List every server.
    List,
    /// Create a Vanilla server.
    Create {
        name: String,
        /// The Minecraft version, such as 1.21.1.
        #[arg(long)]
        version: String,
        /// Accept the Minecraft EULA (https://aka.ms/MinecraftEULA) for this server.
        #[arg(long)]
        accept_eula: bool,
    },
    /// Accept the Minecraft EULA (https://aka.ms/MinecraftEULA) for a server.
    Eula { id: String },
    /// Start a server.
    Start { id: String },
    /// Stop a server, waiting until it has saved and exited.
    Stop { id: String },
    /// Kill a server's Java process now, without saving.
    Kill { id: String },
    /// Stop a server, then start it again.
    Restart { id: String },
}

/// `copal status`.
pub async fn status() -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Health {
        version: String,
        core_id: String,
    }
    let health: Health = Client::new()?.get("/health").await?;
    println!("Core {} is running (id {})", health.version, health.core_id);
    list().await
}

/// `copal servers ...`.
pub async fn servers(command: ServersCommand) -> Result<()> {
    let client = Client::new()?;
    match command {
        ServersCommand::List => list().await,
        ServersCommand::Create {
            name,
            version,
            accept_eula,
        } => {
            let created: CreateServerResponse = client
                .post(
                    "/local/servers",
                    &CreateServerRequest {
                        name,
                        instance: None,
                    },
                )
                .await?;
            let id = created.id.to_string();
            println!("Created server {id}. Installing Minecraft {version}...");
            client
                .post_empty(
                    &format!("/modrinth/v0/servers/{id}/reinstall?hard=false"),
                    &ReinstallRequest::Loader {
                        loader: "vanilla".into(),
                        loader_version: None,
                        game_version: Some(version),
                    },
                )
                .await?;
            if accept_eula {
                client
                    .post_empty(&format!("/local/servers/{id}/eula"), &())
                    .await?;
            }
            println!("Installed. Start it with: copal servers start {id}");
            Ok(())
        }
        ServersCommand::Eula { id } => {
            client
                .post_empty(&format!("/local/servers/{id}/eula"), &())
                .await?;
            println!("EULA accepted for {id}");
            Ok(())
        }
        ServersCommand::Start { id } => {
            power(&client, &id, PowerAction::Start).await?;
            println!("{id} is running");
            Ok(())
        }
        ServersCommand::Stop { id } => {
            power(&client, &id, PowerAction::Stop).await?;
            println!("Stopping {id}...");
            let state = wait_offline(&client, &id).await?;
            println!("{id} is {}", state_name(state));
            Ok(())
        }
        ServersCommand::Kill { id } => {
            power(&client, &id, PowerAction::Kill).await?;
            let state = wait_offline(&client, &id).await?;
            println!("{id} is {}", state_name(state));
            Ok(())
        }
        ServersCommand::Restart { id } => {
            power(&client, &id, PowerAction::Restart).await?;
            println!("Restarting {id}");
            Ok(())
        }
    }
}

async fn list() -> Result<()> {
    let response: ServerGetResponse =
        Client::new()?.get("/modrinth/v0/servers").await?;
    if response.servers.is_empty() {
        println!(
            "No servers yet. Create one with: copal servers create <name> --version <version>"
        );
    }
    for server in response.servers {
        println!(
            "{}  {:<9} {:<10} port {:<5}  {}",
            server.server_id,
            server.power_state.map_or("unknown", state_name),
            server.mc_version.as_deref().unwrap_or("-"),
            server.net.port,
            server.name,
        );
    }
    Ok(())
}

fn state_name(state: ServerPowerState) -> &'static str {
    match state {
        ServerPowerState::Offline => "stopped",
        ServerPowerState::Starting => "starting",
        ServerPowerState::Running => "running",
        ServerPowerState::Stopping => "stopping",
        ServerPowerState::Crashed => "crashed",
    }
}

async fn power(client: &Client, id: &str, action: PowerAction) -> Result<()> {
    client
        .post_empty(
            &format!("/modrinth/v0/servers/{id}/power"),
            &PowerRequest { action },
        )
        .await
}

/// Wait, with no time limit, until the server's Java has exited.
async fn wait_offline(client: &Client, id: &str) -> Result<ServerPowerState> {
    loop {
        let server: Server =
            client.get(&format!("/modrinth/v0/servers/{id}")).await?;
        match server.power_state {
            Some(
                state @ (ServerPowerState::Offline | ServerPowerState::Crashed),
            ) => {
                return Ok(state);
            }
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
}

/// HTTP over the local socket, one connection per request.
pub struct Client {
    paths: Paths,
}

impl Client {
    fn new() -> Result<Self> {
        Ok(Self::at(Paths::new(config::data_dir())))
    }

    /// A client for the Core on the data folder `paths`.
    pub fn at(paths: Paths) -> Self {
        Self { paths }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let body = self.send(Method::GET, path, Bytes::new()).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        let body = self
            .send(Method::POST, path, serde_json::to_vec(body)?.into())
            .await?;
        Ok(serde_json::from_slice(&body)?)
    }

    async fn post_empty(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<()> {
        self.send(Method::POST, path, serde_json::to_vec(body)?.into())
            .await?;
        Ok(())
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Bytes,
    ) -> Result<Bytes> {
        let socket = self.paths.local_socket();
        let stream = local::connect(&self.paths).await.wrap_err_with(|| {
            format!(
                "Core is not running, or this account may not use it (could not open {})",
                socket.display()
            )
        })?;
        let (mut sender, connection) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
        tokio::spawn(connection);
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "copal")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(body))?;
        let response = sender.send_request(request).await?;
        let status = response.status();
        let body = response.into_body().collect().await?.to_bytes();
        if !status.is_success() {
            match serde_json::from_slice::<ApiError>(&body) {
                Ok(error) => bail!("{}", error.description),
                Err(_) => bail!("Core answered {status}"),
            }
        }
        Ok(body)
    }
}
