//! Integration test: the process supervisor, driven against a harmless
//! long-running stub process (standing in for PalServer.exe) through
//! start → status → stop → restart, plus the not-configured path.

use std::path::PathBuf;

use psm_bridge::config::ServerProcessConfig;
use psm_bridge::supervisor::{Supervisor, SupervisorError};

/// A long-running, harmless stub standing in for the game server.
#[cfg(windows)]
fn stub() -> ServerProcessConfig {
    ServerProcessConfig {
        exe: PathBuf::from("cmd"),
        args: vec![
            "/C".into(),
            "ping".into(),
            "-n".into(),
            "60".into(),
            "127.0.0.1".into(),
        ],
    }
}

#[cfg(not(windows))]
fn stub() -> ServerProcessConfig {
    ServerProcessConfig {
        exe: PathBuf::from("sleep"),
        args: vec!["60".into()],
    }
}

#[test]
fn unconfigured_reports_not_configured() {
    let sup = Supervisor::new(None);
    let status = sup.status();
    assert!(!status.configured);
    assert!(!status.running);
    assert!(matches!(sup.start(), Err(SupervisorError::NotConfigured)));
    assert!(matches!(sup.stop(), Err(SupervisorError::NotConfigured)));
    assert!(matches!(sup.restart(), Err(SupervisorError::NotConfigured)));
}

#[test]
fn start_status_stop_lifecycle() {
    let sup = Supervisor::new(Some(stub()));

    // Configured but not yet running.
    let s = sup.status();
    assert!(s.configured);
    assert!(!s.running);
    assert!(s.pid.is_none());

    // Start.
    let s = sup.start().expect("start should succeed");
    assert!(s.running);
    let pid = s.pid.expect("pid present after start");
    assert!(pid > 0);

    // Starting again while running is a conflict.
    assert!(matches!(sup.start(), Err(SupervisorError::AlreadyRunning(_))));
    assert!(sup.status().running);

    // Stop (force-kills the process tree).
    let s = sup.stop().expect("stop should succeed");
    assert!(!s.running);
    assert!(s.pid.is_none());

    // Stopping again is a conflict (nothing to stop).
    assert!(matches!(sup.stop(), Err(SupervisorError::NotRunning)));
}

#[test]
fn restart_relaunches() {
    let sup = Supervisor::new(Some(stub()));
    sup.start().expect("initial start");

    let s = sup.restart().expect("restart should succeed");
    assert!(s.running);
    assert!(s.pid.is_some());

    sup.stop().expect("cleanup stop");
}

#[test]
fn restart_from_stopped_just_starts() {
    let sup = Supervisor::new(Some(stub()));
    // Not running yet — restart should still bring it up.
    let s = sup.restart().expect("restart from stopped should start");
    assert!(s.running);
    sup.stop().expect("cleanup stop");
}
