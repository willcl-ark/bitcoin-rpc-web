pub fn init() {
    if std::env::var_os("RUST_LOG").is_none() {
        return;
    }

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stdout)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_ansi(false)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}
