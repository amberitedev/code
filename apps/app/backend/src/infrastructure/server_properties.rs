//! Server properties - Minecraft server configuration.

use crate::generate_server_properties;

generate_server_properties! {
    (EnableJmxMonitoring, "enable-jmx-monitoring", bool, "Expose JMX metrics"),
    (RconPort, "rcon.port", u16, "Port for RCON connections"),
    (RconPassword, "rcon.password", String, "RCON password"),
    (ServerIp, "server-ip", String, "Bind address"),
    (ServerPort, "server-port", u16, "Server port"),
    (MaxPlayers, "max-players", u32, "Maximum players"),
    (MaxWorldSize, "max-world-size", u32, "Maximum world size"),
    (AllowNether, "allow-nether", bool, "Enable Nether"),
    (EnableCommandBlock, "enable-command-block", bool, "Enable command blocks"),
    (ForceGameMode, "force-gamemode", bool, "Force gamemode"),
    (GenerateStructures, "generate-structures", bool, "Generate structures"),
    (Hardcore, "hardcore", bool, "Hardcore mode"),
    (OnlineMode, "online-mode", bool, "Online mode"),
    (Pvp, "pvp", bool, "Enable PVP"),
    (SpawnNpcs, "spawn-npcs", bool, "Spawn villagers"),
    (SpawnAnimals, "spawn-animals", bool, "Spawn animals"),
    (SpawnMonsters, "spawn-monsters", bool, "Spawn monsters"),
    (ViewDistance, "view-distance", u32, "View distance"),
    (SimulationDistance, "simulation-distance", u32, "Simulation distance"),
    (ResourcePack, "resource-pack", String, "Resource pack URL"),
    (ServerMotd, "motd", String, "Server MOTD"),
}

pub fn get_property_by_key(key: &str) -> Option<&'static ServerProperty> {
    ALL_PROPERTIES.iter().find(|p| p.key == key)
}
