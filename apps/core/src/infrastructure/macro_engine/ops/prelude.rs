/// op: get Amberite Core version string.
#[deno_core::op2]
#[string]
pub fn op_get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

deno_core::extension!(
    amberite_prelude,
    ops = [op_get_version],
    docs = "Amberite prelude ops (version info)"
);
