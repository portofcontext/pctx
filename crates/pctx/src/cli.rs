#[cfg(all(
    not(target_env = "msvc"),
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64"
    )
))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(target_os = "windows")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use clap::Parser;
use log::error;
use pctx::{Cli, utils};

#[tokio::main]
async fn main() {
    // Install default crypto provider for rustls (required for TLS/HTTPS in Deno)
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let cli = Cli::parse();
    // Initialize logger
    utils::logger::init_logger(cli.quiet, cli.verbose);

    if let Err(e) = cli.handle().await {
        error!("{e}");
        std::process::exit(1);
    }
}
