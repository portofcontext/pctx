use log::Level;
use std::io::Write;

pub(crate) fn init_logger(quiet: bool, verbose: u8) {
    let level = if quiet {
        log::Level::Error
    } else if verbose == 0 {
        log::Level::Info
    } else if verbose == 1 {
        log::Level::Debug
    } else {
        log::Level::Trace
    };

    let mut builder = env_logger::builder();

    if level == log::Level::Trace {
        builder.filter_level(level.to_level_filter());
    } else if level == log::Level::Debug {
        builder.filter_module("ptx", level.to_level_filter());
    } else {
        // info, warn, error
        builder
            .filter_module("ptx", level.to_level_filter())
            .format(|buf, record| {
                if record.level() == Level::Info {
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

    let _ = builder.try_init();
}
