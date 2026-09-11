//! WinTab mapping/packet regressions without opening the tablet or shared input stream.
//! Reproduces Graphite's polling context on both x86 and x64; never injects input.
use super::*;

#[test]
fn negative_output_extent_reverses_axis_inside_screen_rectangle() {
    for (raw, expected) in [(0, 1080), (4600, 540), (9200, 0)] {
        assert_eq!(
            coordinate(raw, 0, 9200, 0, (-1080i32) as u32) as i32,
            expected
        );
    }
    // A monitor's negative desktop origin is retained, rather than clipped away.
    assert_eq!(
        coordinate(4600, 0, 9200, (-1080i32) as u32, (-1080i32) as u32) as i32,
        -540
    );
    assert_eq!(coordinate(150, 100, 100, 200, (-300i32) as u32), 350);
    assert_eq!(coordinate(0, 0, 14720, 0, 1920), 0);
    assert_eq!(coordinate(14720, 0, 14720, 0, 1920), 1920);
}

#[test]
fn graphite_polling_packet_keeps_contact_and_pressure_on_canvas() {
    let mut words = default_words();
    words[0] = (words[0] | 1) & !0x0c;
    words[6] = 0x15e6; // Status, time, cursor, buttons, XY, pressure, orientation.
    words[7] = 0;
    words[8] = words[6];
    words[20] = 1920;
    words[21] = (-1080i32) as u32;
    assert!(valid_words(&words));
    let context = Context {
        revision: 0,
        cursor_mask: [255; 16],
        name: "Graphite regression".into(),
        words,
        window: 0,
        enabled: true,
        queue: VecDeque::new(),
        capacity: 512,
        previous: None,
        overflow: false,
        serial: 0,
    };
    for (x, y, screen_x, screen_y) in [
        (0, 0, 0, 0),
        (14720, 0, 1920, 0),
        (7360, 4600, 960, 540),
        (0, 9200, 0, 1080),
        (14720, 9200, 1920, 1080),
    ] {
        for pressure in [1, 1024, 2048, 4097] {
            for flags in [9, 13] {
                // In-range contact, with/without the held eraser.
                let packet = Packet {
                    serial: 42,
                    pen: PenData {
                        x,
                        y,
                        pressure,
                        flags,
                        ..PenData::default()
                    },
                    buttons: 1,
                    changed: SUPPORTED,
                    previous: None,
                    status: 0,
                };
                let bytes = pack(1, &context, &packet);
                assert_eq!(bytes.len(), 40);
                let fields: Vec<i32> = bytes
                    .chunks_exact(4)
                    .map(|b| i32::from_ne_bytes(b.try_into().unwrap()))
                    .collect();
                assert_eq!(
                    fields[0] & 3,
                    0,
                    "Graphite must not cancel an in-range packet"
                );
                assert_eq!(fields[3] & 1, 1, "tip contact must be present");
                assert_eq!((fields[4], fields[5]), (screen_x, screen_y));
                assert_eq!(fields[6], pressure, "preserve pressure variation");
                assert_eq!(
                    fields[0] & 16,
                    if flags & 4 != 0 { 16 } else { 0 },
                    "preserve TPS_INVERT"
                );
                assert_eq!(
                    fields[2],
                    if flags & 4 != 0 { 1 } else { 0 },
                    "preserve eraser cursor identity"
                );
            }
        }
    }
}

#[test]
fn discovery_advertises_usable_contexts_pen_and_eraser_before_open() {
    let value = |c, i| u32::from_ne_bytes(info(c, i, false).try_into().unwrap());
    assert_eq!(value(1, 4), 1);
    assert_eq!(value(1, 5), value(100, 3));
    assert_eq!(value(1, 5), 2);
    assert_eq!(value(1, 6), MAX_CONTEXTS as u32);
    assert_ne!(value(1, 7) & 1, 0);
    for category in [4, 500] {
        assert_ne!(value(category, 2) & 1, 0);
        assert_eq!(value(category, 3), 0);
    }
    for category in [3, 400] {
        assert_eq!(value(category, 2) & 1, 0);
    }
}

#[test]
fn pressure_contact_marks_match_positive_output_for_both_tools() {
    for cursor in [200, 201] {
        let marks = info(cursor, 10, false);
        let release = u32::from_ne_bytes(marks[..4].try_into().unwrap());
        let press = u32::from_ne_bytes(marks[4..].try_into().unwrap());
        assert_eq!(release, 0);
        for pressure in [1, 32, 1024, 2048, 4097] {
            assert!(
                pressure >= press,
                "a client must recognize even light contact"
            );
        }
    }
}

#[test]
fn photoshop_system_context_keeps_strokes_under_the_screen_pointer() {
    // Photoshop 2021 requests WTI_DEFSYSCTX, adds messages, selects
    // 0x1ffc packet data, and scales output coordinates by sixteen while
    // negating Y. It relies on screen units in the returned defaults.
    for category in [4, 500] {
        let data = info(category, 0, true);
        let lc: LogContextW = unsafe { ptr::read_unaligned(data.as_ptr().cast()) };
        let mut words = lc.words;
        let width = words[29];
        let height = words[30];
        words[0] |= 0x0c;
        words[6] = 0x1ffc;
        words[7] = 0;
        words[8] = words[6];
        words[17] *= 16;
        words[18] = (words[18] as i32 * -16) as u32;
        words[20] *= 16;
        words[21] = (words[21] as i32 * -16) as u32;
        assert!(valid_words(&words));
        let context = Context {
            revision: 0,
            cursor_mask: [255; 16],
            name: "System-context client".into(),
            words,
            window: 0,
            enabled: true,
            queue: VecDeque::new(),
            capacity: 128,
            previous: None,
            overflow: false,
            serial: 0,
        };
        for (x, y, screen_x, screen_y) in [
            (0, 0, 0, 0),
            (7360, 4600, width * 8, height * 8),
            (14720, 9200, width * 16, height * 16),
        ] {
            for pressure in [1, 128, 1024, 4097] {
                let packet = Packet {
                    serial: 42,
                    buttons: 1,
                    changed: SUPPORTED,
                    previous: None,
                    status: 0,
                    pen: PenData {
                        x,
                        y,
                        pressure,
                        flags: 9,
                        ..PenData::default()
                    },
                };
                let data = pack(1, &context, &packet);
                let values: Vec<u32> = data
                    .chunks_exact(4)
                    .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
                    .collect();
                assert_eq!(data.len(), 52);
                assert_eq!(
                    (values[5], values[6]),
                    (screen_x, screen_y),
                    "screen-context packets must land under the Windows cursor"
                );
                assert_eq!(values[4] & 1, 1);
                assert_eq!(values[8], pressure as u32);
            }
        }
    }
}

fn context(words: [u32; 33]) -> Context {
    Context {
        revision: 0,
        cursor_mask: [255; 16],
        name: "Conformance".into(),
        words: validated(words, true),
        window: 0,
        enabled: true,
        queue: VecDeque::new(),
        capacity: 128,
        previous: None,
        overflow: false,
        serial: 0,
    }
}
fn sample(time: u32, x: i32, y: i32, pressure: i32) -> PenData {
    PenData {
        time,
        x,
        y,
        pressure,
        flags: if pressure > 0 { 9 } else { 8 },
        ..PenData::default()
    }
}
fn local_state(words: Vec<[u32; 33]>) -> State {
    let mut s = State::default();
    for (index, w) in words.into_iter().enumerate() {
        let id = index + 1;
        s.contexts.insert(id, context(w));
        s.order.insert(0, id);
    }
    overlap_status(&mut s);
    s
}
fn packed_values(c: &Context, p: &Packet) -> Vec<u32> {
    pack(1, c, p)
        .chunks_exact(4)
        .map(|b| u32::from_ne_bytes(b.try_into().unwrap()))
        .collect()
}
#[test]
fn category_zero_only_queries_size_even_with_nonnull_output() {
    for wide in [false, true] {
        let mut guard = [0xa5u8; 8];
        let size = unsafe { query_info(0, 99, guard.as_mut_ptr().cast(), wide) } as usize;
        assert_eq!(guard, [0xa5; 8]);
        for category in [1, 2, 3, 4, 100, 200, 201, 400, 500] {
            assert!(size >= category_info(category, wide).len());
        }
        assert!(size >= 1024);
    }
}
#[test]
fn complete_information_categories_and_name_buffer_sizes() {
    for category in [1, 2, 100, 200, 201] {
        for wide in [false, true] {
            assert!(!category_info(category, wide).is_empty());
        }
    }
    assert_eq!(info(3, 1, false).len(), 40);
    assert_eq!(info(3, 1, true).len(), 80);
    assert_eq!(category_info(3, false).len(), 172);
    assert_eq!(category_info(4, true).len(), 212);
    assert_eq!(info(100, 7, false), uint(0));
    assert_eq!(info(200, 12, false), vec![255]);
    assert_eq!(info(201, 8, false)[..3], [1, 4, 0]);
}
#[test]
fn notifications_honor_event_options_but_always_deliver_lifecycle() {
    for options in [0, 1, 4, 8, 12] {
        assert_eq!(notification_allowed(options, 0), options & 4 != 0);
        assert_eq!(notification_allowed(options, 7), options & 8 != 0);
        for message in 1..=6 {
            assert!(notification_allowed(options, message));
        }
    }
}
#[test]
fn signed_input_and_output_extents_follow_specification_equations() {
    for input in [100i32, -100] {
        for output in [300i32, -300] {
            for (raw, fraction) in [(100, 0.0), (125, 0.25), (200, 1.0)] {
                let f = if input.signum() == output.signum() {
                    fraction
                } else {
                    1.0 - fraction
                };
                assert_eq!(
                    coordinate(raw, 100, input as u32, (-400i32) as u32, output as u32) as i32,
                    -400 + (f * 300.0) as i32
                );
            }
        }
    }
}
#[test]
fn relative_packets_keep_signed_motion_pressure_and_wrapping_time() {
    let mut words = default_words();
    words[6] = 0x584;
    words[7] = 0x584;
    let mut c = context(words);
    post_sample(&mut c, 1, sample(u32::MAX - 2, 1000, 2000, 500), 0);
    post_sample(&mut c, 1, sample(2, 980, 2030, 300), 0);
    let values = packed_values(&c, c.queue.back().unwrap());
    assert_eq!(
        values,
        [5, (-20i32) as u32, (-30i32) as u32, (-200i32) as u32]
    );
    assert_eq!(packed_values(&c, c.queue.front().unwrap()), [0, 0, 0, 0]);
}
#[test]
fn relative_axis_sensitivity_and_inverted_output_are_respected() {
    assert_eq!(relative_coordinate(120, 100, 2 << 16, 100, 100), 40);
    assert_eq!(
        relative_coordinate(120, 100, 1 << 15, 100, (-100i32) as u32) as i32,
        -10
    );
}
#[test]
fn button_masks_choose_events_without_motion_mask_bypass() {
    let mut words = default_words();
    words[8] = 0;
    words[9] = 2;
    words[10] = 2;
    let c = context(words);
    assert!(!selected(&c, sample(1, 100, 100, 500)));
    assert!(selected(
        &c,
        PenData {
            flags: 10,
            ..sample(1, 100, 100, 0)
        }
    ));
    let mut c = c;
    c.previous = Some(PenData {
        flags: 10,
        ..sample(1, 100, 100, 0)
    });
    assert!(selected(&c, sample(2, 100, 100, 0)));
}
#[test]
fn relative_buttons_report_each_selected_transition_with_consecutive_serials() {
    let mut words = default_words();
    words[7] = 0x40;
    let mut c = context(words);
    c.previous = Some(sample(1, 100, 100, 0));
    post_sample(
        &mut c,
        1,
        PenData {
            flags: 11,
            ..sample(2, 100, 100, 500)
        },
        0,
    );
    assert_eq!(
        c.queue
            .iter()
            .map(|p| (p.serial, p.buttons))
            .collect::<Vec<_>>(),
        [(1, 2 << 16), (2, (2 << 16) | 1)]
    );
    post_sample(&mut c, 1, sample(3, 100, 100, 0), 0);
    assert_eq!(c.queue.back().unwrap().buttons, (1 << 16) | 1);
    assert_eq!(c.queue.back().unwrap().serial, 4);
}
#[test]
fn queue_overflow_drops_new_data_preserves_oldest_and_reports_loss() {
    let mut c = context(default_words());
    c.capacity = 2;
    for n in 1..=3 {
        post_sample(&mut c, 1, sample(n, n as i32, 100, 100), 0);
    }
    assert_eq!(c.queue.iter().map(|p| p.serial).collect::<Vec<_>>(), [1, 2]);
    assert_ne!(c.queue.back().unwrap().status & 2, 0);
    c.queue.pop_front();
    post_sample(&mut c, 1, sample(4, 4, 100, 200), 0);
    assert_eq!(c.queue.back().unwrap().serial, 4);
    assert_ne!(c.queue.back().unwrap().status & 2, 0);
}
#[test]
fn packet_status_is_a_snapshot_and_serial_numbers_wrap() {
    let mut c = context(default_words());
    c.serial = u32::MAX - 1;
    c.words[6] = 0x12;
    post_sample(&mut c, 1, sample(1, 1, 1, 1), 0);
    post_sample(&mut c, 1, sample(2, 2, 2, 2), 4);
    assert_eq!(packed_values(&c, c.queue.front().unwrap()), [0, u32::MAX]);
    assert_eq!(packed_values(&c, c.queue.back().unwrap()), [4, 0]);
    c.overflow = true;
    assert_eq!(packed_values(&c, c.queue.front().unwrap())[0], 0);
}
#[test]
fn overlapping_contexts_route_only_to_the_top_eligible_owner() {
    let mut s = local_state(vec![default_words(), default_words()]);
    assert_eq!(s.contexts[&2].words[1], 4);
    assert_eq!(s.contexts[&1].words[1], 2);
    ingest_state(&mut s, sample(1, 100, 100, 0), true);
    assert!(s.contexts[&1].queue.is_empty());
    assert_eq!(s.contexts[&2].queue.len(), 1);
    s.order.reverse();
    overlap_status(&mut s);
    ingest_state(&mut s, sample(2, 200, 200, 0), true);
    assert_eq!(s.contexts[&1].queue.len(), 1);
    assert_eq!(s.contexts[&2].queue.back().unwrap().status & 1, 1);
}
#[test]
fn cropped_context_and_contact_grab_survive_crossing_its_boundary() {
    let mut left = default_words();
    left[14] = 7360;
    let mut right = default_words();
    right[11] = 7361;
    right[14] = 7359;
    let mut s = local_state(vec![left, right]);
    ingest_state(&mut s, sample(1, 1000, 100, 100), true);
    assert_eq!(s.capture, Some(1));
    ingest_state(&mut s, sample(2, 10000, 100, 200), true);
    assert!(s.contexts[&2].queue.is_empty());
    assert_eq!(s.contexts[&1].queue.back().unwrap().status & 9, 9);
    ingest_state(&mut s, sample(3, 10000, 100, 0), true);
    assert_eq!(s.capture, None);
    ingest_state(&mut s, sample(4, 10000, 100, 0), true);
    assert_eq!(s.contexts[&2].queue.len(), 1);
}
#[test]
fn focus_loss_releases_contact_and_resets_the_relative_baseline() {
    let mut s = local_state(vec![default_words()]);
    ingest_state(&mut s, sample(1, 1000, 1000, 100), true);
    ingest_state(&mut s, sample(2, 2000, 2000, 100), false);
    assert_eq!(s.capture, None);
    let last = s.contexts[&1].queue.back().unwrap();
    assert_eq!((last.pen.flags, last.pen.pressure, last.status), (0, 0, 1));
    assert!(s.contexts[&1].previous.is_none());
}
#[test]
fn context_margins_clamp_to_edges_without_scaling_outside_the_output() {
    let mut w = default_words();
    w[11] = 100;
    w[14] = 1000;
    w[0] |= 0x8000;
    assert!(context_sample(&w, sample(1, 83, 100, 0), false).is_none());
    let (p, status) = context_sample(&w, sample(1, 90, 100, 0), false).unwrap();
    assert_eq!((p.x, status), (100, 4));
    w[0] |= 0x4000;
    let (p, status) = context_sample(&w, sample(1, 100, 100, 0), false).unwrap();
    assert_eq!((p.x, status), (116, 4));
}
#[test]
fn save_restore_round_trip_and_corruption_rejection() {
    let mut c = context(default_words());
    c.name = "Ink context".into();
    c.capacity = 512;
    c.words[20] = 3840;
    c.cursor_mask = [0x55; 16];
    let bytes = save_context(&c);
    assert_eq!(bytes.len(), SAVE_SIZE);
    let (name, words, capacity, mask) = restore_context(&bytes).unwrap();
    assert_eq!(mask, c.cursor_mask);
    assert_eq!((name, words, capacity), (c.name, c.words, 512));
    for index in [0, 20, 90, 150, 222, 226] {
        let mut bad = bytes.clone();
        bad[index] ^= 1;
        assert!(restore_context(&bad).is_none());
    }
    assert!(restore_context(&bytes[..100]).is_none());
    let mut legacy = bytes[..224].to_vec();
    legacy[..8].copy_from_slice(b"OCTLWT02");
    legacy.extend(checksum(&legacy).to_le_bytes());
    assert_eq!(restore_context(&legacy).unwrap().3, [255; 16]);
}

#[test]
fn cursor_mask_filters_pen_and_eraser_and_outside_tracking_clamps() {
    let mut c = context(default_words());
    c.cursor_mask = [0; 16];
    c.cursor_mask[0] = 2;
    let pen = sample(100, 100, 300, 9);
    assert!(!selected(&c, pen));
    assert!(selected(
        &c,
        PenData {
            flags: pen.flags | 4,
            ..pen
        }
    ));
    c.words[11] = 200;
    c.words[14] = 500;
    assert!(context_sample(&c.words, pen, false).is_none());
    let (clipped, status) = context_sample(
        &c.words,
        PenData {
            flags: pen.flags | OUT_OF_BOUNDS,
            ..pen
        },
        false,
    )
    .unwrap();
    assert_eq!((clipped.x, status), (200, 4));
}
#[test]
fn all_packet_masks_have_correct_native_stride_and_guard_bytes() {
    let c = context(default_words());
    let p = Packet {
        pen: sample(1, 1, 1, 1),
        serial: 1,
        buttons: 1,
        changed: SUPPORTED,
        previous: None,
        status: 0,
    };
    for mask in 0u32..=SUPPORTED {
        let mut c = Context {
            words: c.words,
            ..context(default_words())
        };
        c.words[6] = mask;
        let size = (0..14)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| match bit {
                0 => size_of::<usize>(),
                12 | 13 => 12,
                _ => 4,
            })
            .sum::<usize>();
        let alignment = if mask & 1 != 0 { size_of::<usize>() } else { 4 };
        assert_eq!(pack(1, &c, &p).len(), size.div_ceil(alignment) * alignment);
    }
}

#[test]
fn stationary_samples_never_end_a_stroke_or_hover() {
    for pressure in [0, 1000] {
        let mut s = local_state(vec![default_words()]);
        for t in 0..20 {
            ingest_state(&mut s, sample(t, 1000, 1000, pressure), true);
        }
        let c = &s.contexts[&1];
        assert_eq!(c.queue.len(), 1);
        assert!(c.previous.unwrap().in_range());
        assert_eq!(c.queue[0].pen.pressure, pressure);
    }
}
#[test]
fn ignored_modes_masks_and_input_bounds_are_validated() {
    let mut w = default_words();
    w[7] = u32::MAX;
    w[8] = u32::MAX;
    w[11] = (-10i32) as u32;
    w[14] = 20000;
    assert!(valid_words(&w));
    let w = validated(w, true);
    assert_eq!(w[7], RELATIVE & w[6]);
    assert_eq!(w[8] & 0x7f, 0);
    assert_eq!((w[11], w[14]), (0, 14720));
}
#[test]
fn grab_is_clipped_and_requires_both_button_masks() {
    let mut w = default_words();
    w[14] = 1000;
    let (p, status) = context_sample(&w, sample(1, 2000, 1000, 500), true).unwrap();
    assert_eq!((p.x, status), (1000, 9));
    w[10] = 0;
    let mut s = local_state(vec![w]);
    ingest_state(&mut s, sample(1, 500, 1000, 500), true);
    assert!(s.capture.is_none());
}

#[test]
fn context_locks_preserve_size_aspect_margin_and_system_mapping() {
    let mut old = default_words();
    old[14] = 4000;
    old[15] = 2000;
    old[2] = 1;
    let mut new = old;
    new[11] = 50;
    new[14] = 5000;
    new[17] = 123;
    let locked = apply_locks(&old, new);
    assert_eq!((locked[11], locked[14], locked[17]), (50, 4000, 123));
    old[2] = 2;
    new = old;
    new[14] = 6000;
    let locked = apply_locks(&old, new);
    assert_eq!((locked[14], locked[15]), (6000, 3000));
    old[2] = 4 | 8 | 16;
    old[0] |= 1 | 0xc000;
    new = old;
    new[23] = 1;
    new[0] &= !0xc000;
    new[29] = 500;
    let locked = apply_locks(&old, new);
    assert_eq!(locked[23], old[23]);
    assert_eq!(locked[0] & 0xc000, 0xc000);
    assert_eq!(locked[29], old[29]);
}

#[test]
fn proximity_distinguishes_hardware_from_context_transitions() {
    let mut p = sample(1, 0, 0, 0);
    assert_eq!(proximity_flags(p), 1);
    p.flags |= HARDWARE_PROXIMITY;
    assert_eq!(proximity_flags(p), 0x10001);
    p.flags = HARDWARE_PROXIMITY;
    assert_eq!(proximity_flags(p), 0x10000);
    p.flags = 0;
    assert_eq!(proximity_flags(p), 0);
}
