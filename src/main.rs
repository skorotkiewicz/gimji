fn main() -> eframe::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("wgpu=warn".parse().unwrap())
                .add_directive("naga=warn".parse().unwrap()),
        )
        .try_init();
    gimji::app::run()
}
