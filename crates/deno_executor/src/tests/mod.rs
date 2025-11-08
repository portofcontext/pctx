use std::sync::Once;

static INIT_CRYPTO: Once = Once::new();

/// Initialize rustls crypto provider for tests that use network operations
fn init_rustls_crypto() {
    INIT_CRYPTO.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

mod default_export_capture;
mod diagnostic_filtering;
mod mcp_client_usage;
mod output_capture;
mod permissions;
mod runtime_execution;
mod type_checking;
