//! Domain layer - Pure business logic and compile-time safety rules.
//! Zero dependencies on Axum, SQLx, or Deno.

pub mod auth;
pub mod instances;
pub mod flavours;
pub mod ports;
pub mod config;
