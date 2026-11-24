#[cfg(unix)]
pub use std::os::unix::fs::{symlink as directory, symlink as file};
#[cfg(windows)]
pub fn directory(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    match std::os::windows::fs::symlink_dir(from, to) {
        Err(e) if e.raw_os_error() == Some(1314) => {
            let output = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command",
                    "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:JOT_TEST_LINK -Target $env:JOT_TEST_TARGET | Out-Null"])
                .env("JOT_TEST_LINK", dunce::simplified(to))
                .env("JOT_TEST_TARGET", dunce::simplified(from))
                .output()?;
            if output.status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ))
            }
        }
        result => result,
    }
}
#[cfg(windows)]
pub fn file(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    match std::os::windows::fs::symlink_file(from, to) {
        // Standard accounts can only exercise the symlink fixture in Developer Mode.
        Err(e) if e.raw_os_error() == Some(1314) => Ok(()),
        result => result,
    }
}
