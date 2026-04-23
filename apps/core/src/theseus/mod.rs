//! Theseus integration for Amberite Core
//! 
//! This module provides server-side modpack installation and management
//! functionality extracted from the Theseus library (modrinth/code).
//! 
//! Focused on Core's needs:
//! - Installing .mrpack files
//! - Managing server profiles/instances  
//! - Downloading Minecraft server files
//! - Launching Minecraft servers

pub mod error;
pub mod pack;
pub mod profile;
pub mod state;
pub mod util;

pub use error::{TheseusError, TheseusResult};
