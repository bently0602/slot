use slot_gfx::{hdmi_rect, OutputScale};

#[test]
fn tv_scaling_preserves_the_canvas_and_centres_it_inside_the_safe_area() {
    assert_eq!(
        hdmi_rect((1280, 720), OutputScale::Fit, 0, 0.0),
        (100, 0, 1080, 720)
    );
    assert_eq!(
        hdmi_rect((1280, 720), OutputScale::Integer, 0, 0.0),
        (280, 120, 720, 480)
    );
    assert_eq!(
        hdmi_rect((1280, 720), OutputScale::Stretch, 5, 0.0),
        (64, 36, 1152, 648)
    );
    assert_eq!(
        hdmi_rect((1920, 1080), OutputScale::Fit, 0, 0.0),
        (150, 0, 1620, 1080)
    );
    assert_eq!(
        hdmi_rect((1920, 1080), OutputScale::Integer, 0, 0.0),
        (240, 60, 1440, 960)
    );
    for window in [(1280, 720), (1920, 1080), (640, 360)] {
        for scale in [OutputScale::Fit, OutputScale::Integer, OutputScale::Stretch] {
            for margin in [0, 5, 10] {
                let (x, y, w, h) = hdmi_rect(window, scale, margin, 0.0);
                assert!(x >= 0 && y >= 0 && w > 0 && h > 0);
                assert_eq!(x * 2 + w, window.0 as i32);
                assert_eq!(y * 2 + h, window.1 as i32);
            }
        }
    }
}

#[test]
fn integer_scaling_falls_back_to_fit_on_small_windows() {
    assert_eq!(
        hdmi_rect((640, 360), OutputScale::Integer, 10, 0.0),
        hdmi_rect((640, 360), OutputScale::Fit, 10, 0.0)
    );
}
