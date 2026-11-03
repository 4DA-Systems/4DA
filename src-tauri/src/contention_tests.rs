use super::*;

/// The measured incident: RE4 in VR renders in a WINDOW, so the OS fullscreen
/// check never fires; the GPU at 100% does.
#[test]
fn a_vr_game_is_contention_by_gpu() {
    assert!(gpu_contended(&[100, 100, 98], 9_644, 16_376, 0));
}

#[test]
fn an_idle_desktop_is_not_contention() {
    assert!(!gpu_contended(&[3, 0, 4], 1_900, 16_376, 0));
}

#[test]
fn one_spike_is_not_contention() {
    assert!(!gpu_contended(&[85, 2, 3], 2_000, 16_376, 0));
    assert!(
        gpu_contended(&[85, 60, 3], 2_000, 16_376, 0),
        "two of three"
    );
}

/// With 4DA's own judge on the GPU, utilization may be ours; what counts is
/// the VRAM other apps hold. Measured 14:29: 15,898 used, judge 8,251.
#[test]
fn with_our_judge_resident_only_other_apps_vram_counts() {
    assert!(
        gpu_contended(&[100, 100, 100], 15_898, 16_376, 8_251),
        "game holds 7.6 GB"
    );
    assert!(
        !gpu_contended(&[100, 100, 100], 10_150, 16_376, 8_251),
        "only the desktop beside it"
    );
}

#[test]
fn only_a_busy_screen_defers_compute() {
    assert!(screen_defers(BusyReason::FullscreenApp));
    assert!(screen_defers(BusyReason::FullscreenWindow));
    assert!(screen_defers(BusyReason::Presentation));
    // Away and locked are the best time for background work; DND, quiet hours
    // and Focus Assist are about interruptions, not load.
    for r in [
        BusyReason::Away,
        BusyReason::ScreenLocked,
        BusyReason::DoNotDisturb,
        BusyReason::QuietHours,
        BusyReason::OsQuietTime,
    ] {
        assert!(!screen_defers(r), "{r:?}");
    }
}

#[test]
fn deferral_is_bounded() {
    let busy = Some(Contention::Gpu {
        utilization_pct: 100,
    });
    assert_eq!(bounded(busy, Some(Duration::from_mins(30))), busy);
    assert_eq!(
        bounded(busy, Some(MAX_DEFER)),
        None,
        "bound reached: run anyway"
    );
    assert_eq!(bounded(busy, None), None, "never fetched: never deferred");
    assert_eq!(bounded(None, Some(Duration::from_mins(5))), None);
}

#[test]
fn nvidia_smi_gpu_line_parses() {
    assert_eq!(
        parse_gpu_line("100, 15898, 16376"),
        Some((100, 15_898, 16_376))
    );
    assert_eq!(parse_gpu_line("3,612,16376"), Some((3, 612, 16_376)));
    assert_eq!(parse_gpu_line("[N/A], 612, 16376"), None);
    assert_eq!(parse_gpu_line(""), None);
}

#[test]
fn reasons_have_stable_tags() {
    assert_eq!(
        Contention::Gpu {
            utilization_pct: 90
        }
        .as_str(),
        "gpu_busy"
    );
    assert_eq!(
        Contention::Screen(BusyReason::FullscreenApp).as_str(),
        "fullscreen_app"
    );
}
