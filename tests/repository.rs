use std::{error::Error, fs, process::Command};

#[test]
fn git_keeps_project_files_and_ignores_local_artifacts() -> Result<(), Box<dyn Error>> {
    let repo = tempfile::tempdir()?;
    let init = Command::new("git")
        .current_dir(repo.path())
        .args(["init", "--quiet", "--template="])
        .output()?;
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    fs::write(
        repo.path().join(".gitignore"),
        include_bytes!("../.gitignore"),
    )?;
    for file in [
        "Cargo.lock",
        "src/main.rs",
        "tests/protocol47.py",
        "data/seed",
        "target/debug/syrust",
        "server.log",
        "tests/tcp.log",
        "tools/__pycache__/rust_mcp.pyc",
    ] {
        let path = repo.path().join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, [])?;
    }

    let files = Command::new("git")
        .current_dir(repo.path())
        .args(["ls-files", "--others", "--exclude-from=.gitignore"])
        .output()?;
    assert!(
        files.status.success(),
        "{}",
        String::from_utf8_lossy(&files.stderr)
    );
    assert_eq!(
        String::from_utf8(files.stdout)?.lines().collect::<Vec<_>>(),
        [
            ".gitignore",
            "Cargo.lock",
            "src/main.rs",
            "tests/protocol47.py"
        ]
    );
    Ok(())
}
