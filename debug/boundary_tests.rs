//! Boundary reach through the complete coordinate pipeline; no device input.
use ctl460_rust::{
    config::Config,
    engine::Engine,
    precision::{output_bounds, PrecisionHold},
    protocol::{Sample, MAX_X, MAX_Y},
    stroke::map_to_screen,
};
#[test]
fn all_modes_keep_uniform_precision_and_normal_screen_reach() {
    for mode in ["off", "on", "auto"] {
        for filter in ["raw", "minimal", "responsive", "legacy"] {
            for smoothing in [false, true] {
                for flowing in [false, true] {
                    let c = Config {
                        handwriting_mode: mode.into(),
                        handwriting_filter: filter.into(),
                        stroke_smoothing: smoothing,
                        flowing_smoothing: flowing,
                        independent_line_controls: true,
                        pen_control: if filter == "legacy" { None } else { Some(30.0) },
                        streamline_amount: 100.0,
                        stabilization_amount: 100.0,
                        circle_smoothing: 100.0,
                        wobble_reduction: 100.0,
                        ..Config::default()
                    };
                    for gain in [1.0, 0.1, 0.69] {
                        for aspect in [false, true] {
                            for left_handed in [false, true] {
                                for tool in [0, 1, 2] {
                                    let c = Config {
                                        button1: "Precision Hold".into(),
                                        precision_gain: [gain * 100.0; 2],
                                        preserve_aspect: aspect,
                                        left_handed,
                                        ..c.clone()
                                    };
                                    for contact in [false, true] {
                                        for (tx, ty) in
                                            [(0, 0), (MAX_X, 0), (0, MAX_Y), (MAX_X, MAX_Y)]
                                        {
                                            let mut e = Engine::new(c.clone()).unwrap();
                                            let mut p = PrecisionHold::default();
                                            if gain != 1.0 {
                                                p.buttons(&c, [true, false]);
                                                p.buttons(&c, [false, false]);
                                            }
                                            let mut last = Default::default();
                                            for i in 0..=600 {
                                                let t = (i as f64 / 200.0).min(1.0);
                                                let s = Sample {
                                                    x: (7360.0 + (f64::from(tx) - 7360.0) * t)
                                                        .round()
                                                        as u16,
                                                    y: (4600.0 + (f64::from(ty) - 4600.0) * t)
                                                        .round()
                                                        as u16,
                                                    tip: contact,
                                                    eraser: tool == 1,
                                                    barrel: tool == 2,
                                                    pressure: if contact { 500 } else { 0 },
                                                    in_range: true,
                                                    position_valid: true,
                                                    ..Default::default()
                                                };
                                                e.push(s, i as f64 * 4.0).unwrap();
                                                last = p.apply(
                                                    e.tick(i as f64 * 4.0),
                                                    output_bounds(3840, 2160, aspect),
                                                );
                                            }
                                            let expected = if left_handed {
                                                (MAX_X - tx, MAX_Y - ty)
                                            } else {
                                                (tx, ty)
                                            };
                                            assert_eq!(map_to_screen(last.x,last.y,3840,2160,aspect),map_to_screen((7360.0+(f64::from(expected.0)-7360.0)*gain).round() as u16,(4600.0+(f64::from(expected.1)-4600.0)*gain).round() as u16,3840,2160,aspect),"{mode} {filter} smoothing={smoothing} flowing={flowing} contact={contact} gain={gain} aspect={aspect} left={left_handed} tool={tool} target={tx},{ty}");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
