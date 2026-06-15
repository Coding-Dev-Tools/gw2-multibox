// Per-account user data directory support via Windows directory junctions.
//
// This module creates a Windows directory junction (reparse point) at the
// standard user data path (e.g. C:\Users\<user>\AppData\Roaming\Guild Wars 2\)
// that points to a per-account folder. Each game instance then sees its own
// Local.dat, GFXSettings.xml, screenshots, addons, etc.
//
// This replicates the technique used by Healix/Gw2Launcher (see
// tools/gw2launcher-src). No DLL injection, no memory modification.
//
// The junction is created BEFORE the game is launched and removed AFTER
// the game exits, so the standard path behaves normally outside of a
// multibox session.

#![cfg(windows)]

use std::io;
use std::path::Path;
use std::process::Command;

/// Create a directory junction at `link` pointing to `target`.
///
/// On Windows, this uses `cmd /c mklink /J`. The link path must be a
/// non-existent or empty directory; the target must be an existing directory.
pub fn create_junction(link: &Path, target: &Path) -> io::Result<()> {
    if !target.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Target is not a directory: {}", target.display()),
        ));
    }
    if link.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Link path already exists: {}", link.display()),
        ));
    }
    if let Some(parent) = link.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "mklink /J failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// Remove a directory junction. Uses `cmd /c rmdir` which works on junctions
/// (does not follow the reparse point).
pub fn remove_junction(link: &Path) -> io::Result<()> {
    if !link.exists() {
        return Ok(());
    }
    let output = Command::new("cmd")
        .args(["/C", "rmdir"])
        .arg(link)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "rmdir failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn create_and_remove_junction() {
        let tmp = env::temp_dir().join("multisbox_test_junction");
        let _ = fs::remove_dir_all(&tmp);
        let target = tmp.join("target");
        let link = tmp.join("link");
        fs::create_dir_all(&target).unwrap();

        create_junction(&link, &target).unwrap();
        assert!(link.exists());

        // Reading from the junction should show the target's contents
        fs::write(target.join("hello.txt"), b"world").unwrap();
        assert!(link.join("hello.txt").exists());

        remove_junction(&link).unwrap();
        assert!(!link.exists());
        assert!(target.exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn create_junction_target_not_dir_fails() {
        let tmp = env::temp_dir().join("multisbox_test_junction_fail");
        let _ = fs::remove_dir_all(&tmp);
        let target = tmp.join("file.txt");
        let link = tmp.join("link");
        fs::create_dir_all(&tmp).unwrap();
        fs::write(&target, b"x").unwrap();

        let result = create_junction(&link, &target);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&tmp);
    }
}
