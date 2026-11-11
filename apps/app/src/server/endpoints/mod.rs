//! The App's own Endpoints, generated to `packages/contracts/ts/app.ts`, which
//! `cargo test -p theseus_gui` keeps fresh. Regenerate it with
//! `cargo test -p theseus_gui -- --ignored write_ts`.

pub mod boot;
pub mod health;

#[cfg(test)]
mod tests {
    use contracts::ts::{config, endpoint};
    use ts_rs::TS;

    use super::{
        boot::{Boot, GetBoot, Platform},
        health::CheckHealth,
    };

    const PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/contracts/ts/app.ts"
    );

    fn file() -> String {
        let cfg = config();
        let declarations = [Boot::decl(&cfg), Platform::decl(&cfg)]
            .map(|declaration| format!("\nexport {declaration}\n"))
            .concat();
        format!(
            "// Generated from apps/app by ts-rs. Do not edit.\n\
             // Regenerate: cargo test -p theseus_gui -- --ignored write_ts\n\n\
             import type {{ Endpoint }} from './contracts'\n{declarations}\n\
             export const health = {{\n\
             \tcheckHealth: {},\n\
             }}\n\n\
             export const boot = {{\n\
             \tgetBoot: {},\n\
             }}\n",
            endpoint::<CheckHealth>(),
            endpoint::<GetBoot>(),
        )
    }

    #[test]
    fn ts_is_up_to_date() {
        let written = std::fs::read_to_string(PATH).unwrap_or_default();
        assert!(
            written.replace("\r\n", "\n") == file(),
            "{PATH} is out of date. Run: cargo test -p theseus_gui -- --ignored write_ts"
        );
    }

    #[test]
    #[ignore = "writes the generated file; run on purpose"]
    fn write_ts() {
        std::fs::write(PATH, file()).unwrap();
    }
}
