//! Records the git commit the app is built from, shown next to its version.

fn main() {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default();
    println!("cargo:rustc-env=MXSC_COMMIT={commit}");
    windows_icon();
    // Rebuild when a new commit is made or checked out.
    for path in [
        "../../.git/HEAD",
        "../../.git/refs/heads",
        "../../.git/packed-refs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
}

/// Gives the Windows program its icon, when MXSC_ICO names an .ico file
/// (the packaging script makes one with `--example icons`).
#[cfg(windows)]
fn windows_icon() {
    println!("cargo:rerun-if-env-changed=MXSC_ICO");
    if let Ok(icon) = std::env::var("MXSC_ICO") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon(&icon)
            .set("ProductName", "MX Shooting Calculator")
            .set("FileDescription", "MX Shooting Calculator");
        resource.compile().expect("the Windows resources compile");
    }
}

#[cfg(not(windows))]
fn windows_icon() {}
