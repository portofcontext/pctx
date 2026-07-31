use std::io::Write;

const WHITELISTED_CRATES: &[&str] = &[
    "pctx",
    "pctx_mcp_server",
    "pctx_session_server",
    "pctx_config",
    "pctx_executor",
    "pctx_codegen",
    "pctx_registry",
];

pub(crate) fn default_env_filter(level: &str) -> String {
    let mut filters: Vec<String> = WHITELISTED_CRATES
        .iter()
        .map(|crate_name| format!("{crate_name}={level}"))
        .collect();

    // Set default level for all other crates to warn
    filters.insert(0, "warn".to_string());

    filters.join(",")
}

pub(crate) fn init_cli_logger(verbose: u8, quiet: bool) {
    let level_str = if quiet {
        "warn"
    } else if verbose == 0 {
        "info"
    } else if verbose == 1 {
        "debug"
    } else {
        "trace"
    };

    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(default_env_filter(level_str)),
    );

    // For INFO and below, only include/colorize WARN and ERROR levels
    if ["info", "warn", "error"].contains(&level_str) {
        builder.format(|buf, record| {
            if record.level() == tracing::log::Level::Info {
                writeln!(buf, "{}", record.args())
            } else {
                let log_style = buf.default_level_style(record.level());
                writeln!(
                    buf,
                    "{log_style}[{}]{log_style:#} {}",
                    record.level(),
                    record.args()
                )
            }
        });
    }

    if let Err(e) = builder.try_init() {
        eprintln!("pctx: Failed initializing env_logger: {e:?}");
    }
}
