// `sqlx::migrate!` embeds the migrations at compile time: rebuild when one changes.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
