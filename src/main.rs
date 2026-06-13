fn main() -> eframe::Result<()> {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
    gimji::app::run()
}
