use aim_telemetry::{sample_session, write_aft};
use std::process::Command;

#[test]
fn test_aft_dump_summary_cli() {
    let session = sample_session();
    let bytes = write_aft(&session).expect("Failed to write aft");

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("test_sample_{}.aft", std::process::id()));
    std::fs::write(&file_path, &bytes).expect("Failed to write sample file");

    let output = Command::new(env!("CARGO_BIN_EXE_aft-dump"))
        .arg("--summary")
        .arg(&file_path)
        .output()
        .expect("Failed to execute aft-dump");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("=== AFT Summary:"));
    assert!(stdout.contains("Scenario: assess.v1.flick_static"));
    assert!(stdout.contains("DPI: 800"));
    assert!(stdout.contains("Sens: 0.35"));
    assert!(stdout.contains("Polling Hz: 1000"));
    assert!(stdout.contains("Quality Flags: CLEAN"));
    assert!(stdout.contains("Shots: 1, Hits: 1, Misses: 0, Accuracy: 100.0%"));
    assert!(stdout.contains("Trial #1: 1.0 ms"));

    let _ = std::fs::remove_file(&file_path);
}
