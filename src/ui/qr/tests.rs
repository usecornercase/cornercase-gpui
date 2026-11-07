use super::{Matrix, QUIET, Run, fit, runs_of};

const ADDRESS: &str = "https://ana.tail0.ts.net:8443/pair?t=0123456789abcdef0123456789abcdef";

fn matrix() -> Matrix {
    Matrix::of(ADDRESS).expect("the address fits a code")
}

fn dark(matrix: &Matrix, x: u16, y: u16) -> bool {
    matrix.dark[usize::from(y) * usize::from(matrix.width) + usize::from(x)]
}

fn has_finder(matrix: &Matrix, left: u16, top: u16) -> bool {
    (0..7).all(|y| {
        (0..7).all(|x| {
            let ring = x == 0 || x == 6 || y == 0 || y == 6;
            let core = (2..=4).contains(&x) && (2..=4).contains(&y);
            dark(matrix, left + x, top + y) == (ring || core)
        })
    })
}

#[test]
fn a_code_has_its_finder_patterns_in_three_corners() {
    let matrix = matrix();
    let far = matrix.width - 7;
    assert!(has_finder(&matrix, 0, 0));
    assert!(has_finder(&matrix, far, 0));
    assert!(has_finder(&matrix, 0, far));
    assert!(!has_finder(&matrix, far, far));
}

#[test]
fn the_ground_keeps_a_quiet_zone_of_four_modules_on_each_side() {
    let matrix = matrix();
    assert_eq!(QUIET, 4);
    assert_eq!(matrix.side(), matrix.width + 8);
}

#[test]
fn the_runs_draw_every_dark_module_and_no_other() {
    let matrix = matrix();
    let mut drawn = vec![false; matrix.dark.len()];
    for run in matrix.runs() {
        for x in run.x..run.x + run.length {
            let at = usize::from(run.y) * usize::from(matrix.width) + usize::from(x);
            assert!(!drawn[at], "a module is drawn once");
            drawn[at] = true;
        }
    }
    assert_eq!(drawn, matrix.dark);
}

#[test]
fn a_row_gives_one_run_per_stretch_of_dark_modules() {
    let row = [true, true, false, true, false, false, true];
    let runs = runs_of(3, &row);
    assert_eq!(runs, vec![Run { x: 0, y: 3, length: 2 }, Run { x: 3, y: 3, length: 1 }, Run { x: 6, y: 3, length: 1 }]);
}

#[test]
fn a_module_is_a_whole_number_of_device_pixels_at_any_scale() {
    for scale in [1., 1.25, 1.5, 2., 3.] {
        let placed = fit(49, 280., scale);
        let device = placed.module * scale;
        assert!((device - device.round()).abs() < 1e-3, "{device} device pixels at {scale}");
        assert!(placed.side <= 280.);
    }
}

#[test]
fn a_module_takes_all_the_room_it_can() {
    assert!((fit(49, 280., 1.).module - 5.).abs() < 1e-3);
    assert!((fit(49, 280., 2.).module - 5.5).abs() < 1e-3);
}

#[test]
fn a_room_too_small_still_gives_a_module_one_pixel() {
    assert!((fit(185, 100., 1.).module - 1.).abs() < 1e-3);
}

#[test]
fn a_text_too_long_for_a_code_has_none() {
    assert_eq!(Matrix::of(&"x".repeat(4000)), None);
}
