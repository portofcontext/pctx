use std::env;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=bin/");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Determine the platform-specific binary name
    let (platform, extension) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", "aarch64") => ("darwin-arm64", ""),
        ("macos", "x86_64") => ("darwin-x64", ""),
        ("linux", "aarch64") => ("linux-arm64", ""),
        ("linux", "x86_64") => ("linux-x64", ""),
        ("windows", "aarch64") => ("win32-arm64", ".exe"),
        ("windows", "x86_64") => ("win32-x64", ".exe"),
        _ => {
            eprintln!("Warning: Unsupported platform {target_os}-{target_arch}");
            eprintln!("TypeScript type checking will not be available.");
            return Ok(());
        }
    };

    let binary_name = format!("tsgo-{platform}{extension}");
    let binary_path = manifest_dir.join("bin").join(&binary_name);

    if !binary_path.exists() {
        eprintln!(
            "Warning: TypeScript binary not found at {}",
            binary_path.display()
        );
        eprintln!("TypeScript type checking will not be available.");
        return Ok(());
    }

    // Set executable permissions on Unix platforms
    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
    }

    println!("cargo:rustc-env=TSGO_BINARY_PATH={}", binary_path.display());

    Ok(())
}
