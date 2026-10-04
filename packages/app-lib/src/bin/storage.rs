fn main() -> eyre::Result<()> {
    let configuration = theseus::storage::Configuration::from_env()?;
    actix_web::rt::System::new().block_on(theseus::storage::run(configuration))
}
