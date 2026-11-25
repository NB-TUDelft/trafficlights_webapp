// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! The `trafficlights-webapp` binary: `--version`, `--healthcheck`, argument
//! errors and, on Unix, a clean exit on SIGTERM (Docker's stop signal).

use std::process::{Command, Output, Stdio};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trafficlights-webapp"))
}

fn run(command: &mut Command) -> Output {
    command.output().expect("run trafficlights-webapp")
}

/// A loopback address that nothing is listening on.
fn unused_bind() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port");
    let address = listener.local_addr().expect("local address");
    drop(listener);
    address.to_string()
}

#[test]
fn version_prints_the_license_and_source_link() {
    let output = run(binary().arg("--version"));
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf-8");
    assert!(text.starts_with("trafficlights-webapp 0.1.0"), "{text}");
    assert!(text.contains("License: AGPL-3.0-or-later"), "{text}");
    assert!(
        text.contains("Source: https://github.com/NB-TUDelft/trafficlights_webapp"),
        "{text}"
    );
}

#[test]
fn healthcheck_fails_when_no_server_is_listening() {
    let output = run(binary().arg("--healthcheck").env("BIND", unused_bind()));
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn unknown_arguments_are_a_usage_error() {
    let output = run(binary().arg("--bogus"));
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stderr).expect("utf-8");
    assert!(text.contains("unknown argument: --bogus"), "{text}");
    assert!(text.contains("usage: trafficlights-webapp"), "{text}");
}

#[cfg(unix)]
#[test]
fn sigterm_stops_the_server_cleanly() {
    use std::time::{Duration, Instant};

    let bind = unused_bind();
    let mut server = binary()
        .env("BIND", &bind)
        .env("LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");

    let started = Instant::now();
    while !trafficlights_server::healthcheck(&bind) {
        if started.elapsed() > Duration::from_secs(20) {
            server.kill().ok();
            panic!("server never became healthy");
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // The shell's built-in kill: slim images lack procps and its kill binary.
    let sent = Command::new("sh")
        .args(["-c", &format!("kill -TERM {}", server.id())])
        .status()
        .expect("sh");
    assert!(sent.success());

    let stopping = Instant::now();
    loop {
        if let Some(status) = server.try_wait().expect("wait") {
            assert!(status.success(), "unclean exit after SIGTERM: {status}");
            return;
        }
        if stopping.elapsed() > Duration::from_secs(5) {
            server.kill().ok();
            panic!("server ignored SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
