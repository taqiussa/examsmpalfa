use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn spawn_child(mut cmd: Command, name: &'static str) -> std::io::Result<std::process::Child> {
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let child = cmd.spawn()?;
    eprintln!("[watch] started {name}");
    Ok(child)
}

fn main() -> std::io::Result<()> {
    let mut be_cmd = Command::new("cargo");
    be_cmd
        .arg("watch")
        .arg("--workdir")
        .arg(".")
        .arg("-w")
        .arg("src")
        .arg("-w")
        .arg("templates")
        .arg("-x")
        .arg("run --bin examsmkmifda");
    let mut be = spawn_child(be_cmd, "backend (cargo watch)")?;

    let mut fe_cmd = Command::new("npm");
    fe_cmd.arg("--prefix").arg("fe").arg("run").arg("dev");
    let mut fe = spawn_child(fe_cmd, "frontend (vite)")?;

    let (who, status) = loop {
        if let Some(status) = be.try_wait()? {
            break ("backend", status);
        }
        if let Some(status) = fe.try_wait()? {
            break ("frontend", status);
        }
        thread::sleep(Duration::from_millis(200));
    };

    eprintln!("[watch] {who} exited with {status}");

    // Ensure the other process is stopped.
    match who {
        "backend" => {
            let _ = fe.kill();
        }
        "frontend" => {
            let _ = be.kill();
        }
        _ => {}
    }

    let code = status.code().unwrap_or(1);
    std::process::exit(code);
}
