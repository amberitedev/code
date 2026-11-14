//! Small integration hooks for the self-hosted account and sharing services.
//! Public content requests continue to use Modrinth's existing endpoints.
pub mod accounts;
pub(crate) mod pool;
pub(crate) mod public_content;
