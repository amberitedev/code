// The content store clone reads Modrinth's API address at compile time, as app-lib does from its
// `.env`. Glue for `upstream/store` (`ports.toml`).
fn main() {
    println!("cargo::rustc-env=MODRINTH_API_URL=https://api.modrinth.com/v2/");
}
