use super::placement::{Area, centered, top_edge_on_screen};
use super::{Bounds, Observation, SavedWindowState, fold};

const NORMAL: Bounds = Bounds {
    x: 400,
    y: 300,
    width: 2000,
    height: 1200,
};

const MAXIMIZED: Bounds = Bounds {
    x: 114,
    y: 60,
    width: 5006,
    height: 2820,
};

fn observation(bounds: Bounds) -> Observation {
    Observation {
        minimized: false,
        fullscreen: false,
        maximized: false,
        bounds,
    }
}

fn saved(bounds: Bounds, maximized: bool) -> Option<SavedWindowState> {
    Some(SavedWindowState { bounds, maximized })
}

#[test]
fn a_normal_window_is_taken_as_it_is() {
    assert_eq!(fold(None, observation(NORMAL)), saved(NORMAL, false));
    assert_eq!(
        fold(saved(MAXIMIZED, true), observation(NORMAL)),
        saved(NORMAL, false)
    );
}

#[test]
fn a_maximized_window_keeps_the_bounds_it_had_before() {
    let maximized = Observation {
        maximized: true,
        ..observation(MAXIMIZED)
    };
    assert_eq!(fold(saved(NORMAL, false), maximized), saved(NORMAL, true));
}

#[test]
fn a_window_maximized_from_the_start_falls_back_to_its_maximized_bounds() {
    let maximized = Observation {
        maximized: true,
        ..observation(MAXIMIZED)
    };
    assert_eq!(fold(None, maximized), saved(MAXIMIZED, true));
}

#[test]
fn minimized_and_fullscreen_windows_change_nothing() {
    let minimized = Observation {
        minimized: true,
        ..observation(Bounds {
            x: -32000,
            y: -32000,
            width: 160,
            height: 28,
        })
    };
    let fullscreen = Observation {
        fullscreen: true,
        ..observation(MAXIMIZED)
    };
    for latest in [None, saved(NORMAL, false), saved(NORMAL, true)] {
        assert_eq!(fold(latest, minimized), latest);
        assert_eq!(fold(latest, fullscreen), latest);
    }
}

#[test]
fn an_empty_window_changes_nothing() {
    let empty = observation(Bounds { width: 0, ..NORMAL });
    assert_eq!(fold(saved(NORMAL, false), empty), saved(NORMAL, false));
}

const MAIN: Area = Area {
    x: 0,
    y: 0,
    width: 5120,
    height: 2880,
};

/// A second monitor to the left of the main one.
const LEFT: Area = Area {
    x: -1920,
    y: 0,
    width: 1920,
    height: 1080,
};

#[test]
fn bounds_on_a_connected_monitor_are_on_screen() {
    assert!(top_edge_on_screen(NORMAL, &[MAIN]));
    let on_left = Bounds {
        x: -1800,
        y: 100,
        width: 1000,
        height: 700,
    };
    assert!(top_edge_on_screen(on_left, &[MAIN, LEFT]));
}

#[test]
fn bounds_on_an_unplugged_monitor_are_off_screen() {
    let on_left = Bounds {
        x: -1800,
        y: 100,
        width: 1000,
        height: 700,
    };
    assert!(!top_edge_on_screen(on_left, &[MAIN]));
    assert!(!top_edge_on_screen(NORMAL, &[]));
}

#[test]
fn a_title_bar_above_every_screen_is_off_screen() {
    let above = Bounds { y: -40, ..NORMAL };
    assert!(!top_edge_on_screen(above, &[MAIN]));
}

#[test]
fn a_sliver_of_title_bar_is_not_enough() {
    let mostly_right = Bounds {
        x: 5120 - 40,
        ..NORMAL
    };
    assert!(!top_edge_on_screen(mostly_right, &[MAIN]));
    let enough = Bounds {
        x: 5120 - 100,
        ..NORMAL
    };
    assert!(top_edge_on_screen(enough, &[MAIN]));
}

#[test]
fn centering_keeps_the_size_that_fits() {
    let work_area = Area {
        x: 114,
        y: 60,
        width: 5006,
        height: 2820,
    };
    let bounds = Bounds { x: -5000, ..NORMAL };
    assert_eq!(
        centered(bounds, work_area),
        Bounds {
            x: 114 + 1503,
            y: 60 + 810,
            width: 2000,
            height: 1200,
        }
    );
}

#[test]
fn centering_shrinks_a_window_larger_than_the_screen() {
    let bounds = Bounds {
        x: -7000,
        y: 0,
        width: 6000,
        height: 3000,
    };
    assert_eq!(
        centered(bounds, LEFT),
        Bounds {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        }
    );
}
