use super::*;

#[test]
fn diagnostics_are_captured_and_surfaced_as_a_toast() {
    let _serial = sequential();
    let _guard = DiagnosticsGuard::install();
    let mut app = sample_app();

    // The regression: a cleaner reporting a failure used to `eprintln!`
    // straight onto the alternate screen, tearing the frame apart in the
    // middle of a run.
    database::diag::warn("cleaner: remove_file C:\\cache\\a.tmp: access denied");
    assert!(
        !draw(&mut app, 100, 30).contains("access denied"),
        "a diagnostic must never be part of a frame",
    );

    // The next tick turns it into a toast instead.
    app.tick();
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("access denied"), "{screen}");

    // Drained, so an expired toast does not bring it back.
    app.toast = None;
    app.tick();
    assert!(!draw(&mut app, 100, 30).contains("access denied"));

    // A run that fails on every entry must not bury the interface either. The
    // queue is capped, so only `DIAGNOSTIC_HISTORY` messages survive and the
    // toast counts the rest it dropped.
    for i in 0..40 {
        database::diag::warn(format!("cleaner: failure {i}"));
    }
    app.tick();
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("failure 39"), "{screen}");
    assert!(
        screen.contains(&format!("(+{} more)", DIAGNOSTIC_HISTORY - 1)),
        "{screen}",
    );

    // `info` is surfaced too — image optimization reports on every file.
    app.toast = None;
    database::diag::info("[image_optimize] WRITTEN saved=4096");
    app.tick();
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("[info]"), "{screen}");
    assert!(screen.contains("saved=4096"), "{screen}");
}
