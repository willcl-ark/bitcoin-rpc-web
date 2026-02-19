pub fn init() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("bitcoin_rpc_web=info"));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stdout)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_ansi(false)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}
