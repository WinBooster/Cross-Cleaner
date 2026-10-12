use super::*;

#[test]
fn no_update_dialog_without_a_pending_release() {
    let mut app = sample_app();
    assert!(matches!(
        crate::pages::update::prompt(&app),
        crate::pages::update::Prompt::Hidden
    ));
    let screen = draw(&mut app, 100, 30);
    assert!(!screen.contains("download and install"), "{screen}");
}

#[test]
fn the_dialog_is_hidden_until_the_user_asks_for_it() {
    let (mut app, _rx) = app_with_update();
    assert!(matches!(
        crate::pages::update::prompt(&app),
        crate::pages::update::Prompt::Hidden
    ));
    press(&mut app, KeyCode::Char('U'));
    assert!(app.update_open);
    assert!(matches!(
        crate::pages::update::prompt(&app),
        crate::pages::update::Prompt::Offer
    ));
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("download and install"), "{screen}");
    assert!(screen.contains("v9.9.9 is available"), "{screen}");
}

#[test]
fn a_release_without_a_binary_offers_only_the_release_page() {
    let (mut app, _rx) = app_with_update();
    app.update_release = Some(NewRelease {
        asset_url: None,
        ..release()
    });
    app.update_open = true;
    let screen = draw(&mut app, 100, 30);
    assert!(
        screen.contains("No terminal binary"),
        "the warning must explain why:\n{screen}",
    );
    assert!(
        !screen.contains("download and install"),
        "a download would fail:\n{screen}",
    );
    assert!(screen.contains("release page"), "{screen}");
}

/// A release this build can install must download itself, the way the window
/// frontend does — waiting for a key press is what made the terminal app look
/// like it had no updater at all.
#[test]
fn a_found_release_starts_downloading_without_a_keypress() {
    let _serial = sequential();
    let (mut app, rx) = app_with_check(Some(release()));
    app.tick();

    match rx.try_recv().expect("the worker was asked to install") {
        UpdaterCommand::Install(release) => assert_eq!(release.version, "9.9.9"),
        UpdaterCommand::Restart => panic!("expected an install, got a restart"),
    }
    assert!(
        matches!(
            updater::current(&app.updater_state),
            UpdateStage::Downloading { .. }
        ),
        "the opening stage is published synchronously",
    );
    // The footer reports it, but the dialog stays shut: a modal that ate the
    // keyboard for the length of a download would lock the user out of the
    // app they were cleaning with.
    assert!(!app.update_open, "not modal while it downloads");
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("updating to v9.9.9"), "{screen}");
}

/// The dialog waits for the one moment the user actually has to decide, then
/// stays down once answered.
#[test]
fn the_dialog_opens_when_the_worker_needs_an_answer_and_not_before() {
    let _serial = sequential();
    let (mut app, _rx) = app_with_check(Some(release()));
    app.tick();
    assert!(!app.update_open, "the first byte needs no decision");

    updater::publish(
        &app.updater_state,
        UpdateStage::Installed {
            version: "9.9.9".to_string(),
        },
    );
    app.tick();
    assert!(app.update_open, "restarting is the user's call");
    assert!(
        app.update_progress.is_none(),
        "the footer hands its line over to the dialog",
    );
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Restart to use it"), "{screen}");

    press(&mut app, KeyCode::Esc);
    assert!(!app.update_open);
    app.tick();
    assert!(
        !app.update_open,
        "an update the user answered must not come back",
    );
}

/// A failed automatic download is reported the same way: not as a toast that
/// fades, but as the dialog offering a retry.
#[test]
fn a_failed_automatic_download_asks_for_a_decision_too() {
    let _serial = sequential();
    let (mut app, _rx) = app_with_check(Some(release()));
    app.tick();
    updater::publish(
        &app.updater_state,
        UpdateStage::Failed {
            version: "9.9.9".to_string(),
            error: "connection reset".to_string(),
        },
    );
    app.tick();
    assert!(app.update_open);
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("connection reset"), "{screen}");
    assert!(screen.contains("retry"), "{screen}");
}

/// Nothing to install means nothing to start: the release page is all this
/// build can honestly offer, and it has to say so.
/// A retry started from the dialog is still this app's own update, so the
/// footer follows the second attempt instead of freezing on the first.
#[test]
fn a_retry_keeps_reporting_progress_in_the_footer() {
    let _serial = sequential();
    let (mut app, _rx) = app_with_check(Some(release()));
    app.tick();
    updater::publish(
        &app.updater_state,
        UpdateStage::Failed {
            version: "9.9.9".to_string(),
            error: "connection reset".to_string(),
        },
    );
    app.tick();
    assert!(app.update_open);

    press(&mut app, KeyCode::Char('r'));
    assert!(
        updater::current(&app.updater_state).is_running(),
        "`r` on a failure starts a fresh download",
    );
    app.tick();
    assert!(
        app.update_progress
            .as_deref()
            .is_some_and(|line| line.contains("updating to")),
        "the footer follows the retry: {:?}",
        app.update_progress,
    );

    updater::publish(
        &app.updater_state,
        UpdateStage::Installed {
            version: "9.9.9".to_string(),
        },
    );
    app.tick();
    assert!(app.update_progress.is_none(), "the download is over");
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Restart to use it"), "{screen}");
}

/// The regression behind "the updater never did anything": the tick is far
/// faster than the request, so the first ticks find an empty channel. Treating
/// "not answered yet" as "will never answer" threw the receiver away ~100 ms in,
/// long before the ~1.5 s response could arrive — and the release was then never
/// seen at all.
#[test]
fn a_check_that_is_still_in_flight_keeps_being_polled() {
    let _serial = sequential();
    let mut app = sample_app();
    // A live sender that has not reported yet, which is exactly what the first
    // ticks see.
    let (tx, check) = std::sync::mpsc::channel();
    app.update_receiver = Some(check);

    for _ in 0..5 {
        app.tick();
    }
    assert!(
        app.update_receiver.is_some(),
        "an unanswered check must still be polled",
    );
    assert!(app.toast.is_none(), "still in flight is not a failure");

    // And when it does answer, the answer is still picked up.
    tx.send(Ok(Some(release()))).expect("receiver alive");
    app.tick();
    assert!(app.update_receiver.is_none(), "polling stops once answered");
    // `sample_app` has no self-update worker, so this build takes the
    // release-page path — the point is that the release was seen at all.
    assert_eq!(
        app.update_release.as_ref().map(|r| r.version.as_str()),
        Some("9.9.9"),
        "the answer that arrived late must still be acted on",
    );
}

/// A check that failed must say so. GitHub rate-limits unauthenticated
/// clients, and a silently dropped error is indistinguishable from "you are
/// already on the newest version".
#[test]
fn a_failed_check_is_reported_instead_of_being_dropped() {
    let _serial = sequential();
    let mut app = sample_app();
    let (tx, check) = std::sync::mpsc::channel();
    tx.send(Err(
        "Failed to request latest release: status 403".to_string()
    ))
    .expect("receiver alive");
    app.update_receiver = Some(check);
    app.tick();
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Update check failed"), "{screen}");
    assert!(screen.contains("403"), "the reason must survive: {screen}");
    assert!(!app.auto_update, "nothing was started");

    // Polling stops, so the toast is not refreshed every tick.
    app.tick();
    assert!(app.update_receiver.is_none(), "not polled again");
}

#[test]
fn a_check_that_found_nothing_stays_quiet() {
    let _serial = sequential();
    let mut app = sample_app();
    let (tx, check) = std::sync::mpsc::channel();
    tx.send(Ok(None)).expect("receiver alive");
    app.update_receiver = Some(check);
    app.tick();
    assert!(app.toast.is_none(), "already up to date is not news");
    assert!(app.update_receiver.is_none(), "not polled again");
}

#[test]
fn a_release_this_build_cannot_install_only_points_at_the_page() {
    let _serial = sequential();
    let (mut app, rx) = app_with_check(Some(NewRelease {
        asset_url: None,
        ..release()
    }));
    app.tick();
    assert!(
        rx.try_recv().is_err(),
        "a release with no binary must not be handed to the worker",
    );
    assert!(!app.auto_update, "nothing was started");
    assert!(!app.update_open);
    assert!(app.update_progress.is_none());
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("press O for the release page"), "{screen}");
}

#[test]
fn a_build_without_a_worker_only_points_at_the_page() {
    let _serial = sequential();
    let mut app = sample_app();
    let (tx, check) = std::sync::mpsc::channel();
    tx.send(Ok(Some(release()))).expect("receiver alive");
    app.update_receiver = Some(check);
    app.tick();
    assert!(!app.auto_update, "no worker, so no download");
    assert!(!app.update_open);
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("press O for the release page"), "{screen}");
}

#[test]
fn d_starts_the_download_and_the_bar_shows_progress() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    press(&mut app, KeyCode::Char('d'));
    // The opening stage is published synchronously, so the dialog never
    // shows an idle bar while the worker picks the command up.
    let stage = updater::current(&app.updater_state);
    assert!(
        matches!(stage, UpdateStage::Downloading { .. }),
        "{stage:?}"
    );
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("downloading v9.9.9"), "{screen}");
    assert!(!screen.contains("download and install"), "{screen}");
}

#[test]
fn d_hands_the_release_to_the_worker() {
    let (mut app, rx) = app_with_update();
    app.update_open = true;
    press(&mut app, KeyCode::Char('d'));
    let command = rx.try_recv().expect("the worker was asked to install");
    match command {
        UpdaterCommand::Install(release) => {
            assert_eq!(release.version, "9.9.9");
            assert!(release.has_asset());
        }
        UpdaterCommand::Restart => panic!("expected an install, got a restart"),
    }
}

#[test]
fn a_running_download_cannot_be_dismissed() {
    let (mut app, rx) = app_with_update();
    app.update_open = true;
    press(&mut app, KeyCode::Char('d'));
    assert!(
        rx.try_recv().is_ok(),
        "the first download is the only one the worker may get",
    );
    press(&mut app, KeyCode::Esc);
    assert!(
        app.update_open,
        "closing mid-download would strand the user on the old binary",
    );
    // ...`r` must not relaunch on top of it either, and `d` must not start a
    // second download writing to the same executable.
    press(&mut app, KeyCode::Char('r'));
    assert!(updater::current(&app.updater_state).is_running());
    press(&mut app, KeyCode::Char('d'));
    assert!(
        rx.try_recv().is_err(),
        "only the first install may reach the worker",
    );
}

#[test]
fn an_installed_update_offers_a_restart_and_later() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    updater::publish(
        &app.updater_state,
        UpdateStage::Installed {
            version: "9.9.9".to_string(),
        },
    );
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Restart to use it?"), "{screen}");
    assert!(screen.contains("restart now"), "{screen}");
    // `Later` dismisses it.
    press(&mut app, KeyCode::Esc);
    assert!(!app.update_open);
}

#[test]
fn r_after_installing_sends_the_restart_command() {
    let (mut app, rx) = app_with_update();
    app.update_open = true;
    updater::publish(
        &app.updater_state,
        UpdateStage::Installed {
            version: "9.9.9".to_string(),
        },
    );
    press(&mut app, KeyCode::Char('r'));
    assert!(
        matches!(rx.try_recv(), Ok(UpdaterCommand::Restart)),
        "expected a restart command",
    );
}

#[test]
fn a_failed_update_offers_a_retry() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    updater::publish(
        &app.updater_state,
        UpdateStage::Failed {
            version: "9.9.9".to_string(),
            error: "connection reset".to_string(),
        },
    );
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("connection reset"), "{screen}");
    assert!(screen.contains("retry"), "{screen}");
}

#[test]
fn retry_after_a_failure_starts_the_download_again() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    updater::publish(
        &app.updater_state,
        UpdateStage::Failed {
            version: "9.9.9".to_string(),
            error: "connection reset".to_string(),
        },
    );
    press(&mut app, KeyCode::Char('r'));
    assert!(updater::current(&app.updater_state).is_running());
}

#[test]
fn a_build_without_a_worker_cannot_start_an_install() {
    let mut app = sample_app();
    app.update_release = Some(release());
    app.update_open = true;
    press(&mut app, KeyCode::Char('d'));
    // No worker, so nothing may claim to be downloading.
    assert!(!updater::current(&app.updater_state).is_running());
    let screen = draw(&mut app, 100, 30);
    assert!(screen.contains("Self-update is not available"), "{screen}");
}

#[test]
fn the_update_dialog_renders_on_every_page() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    for page in [
        Page::Main,
        Page::ProgramSelection,
        Page::Clearing,
        Page::Results,
        Page::Settings,
    ] {
        app.state.current_page = page;
        let screen = draw(&mut app, 100, 30);
        assert!(
            screen.contains("download and install"),
            "{page:?}\n{screen}"
        );
    }
}

#[test]
fn the_update_dialog_survives_a_tiny_terminal() {
    let (mut app, _rx) = app_with_update();
    app.update_open = true;
    let _ = draw(&mut app, 20, 8);
    let _ = draw(&mut app, 10, 4);
}
