use super::*;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn a_colour_reads_as_six_or_eight_hex_digits() {
    let plain = parse_hex("#005fff").expect("six digits");
    assert_eq!(rgb_bytes(plain), [0x00, 0x5f, 0xff]);
    assert!(close(plain.a, 1.));
    let clear = parse_hex("#ffffff80").expect("eight digits");
    assert!(close(clear.a, 128. / 255.));
}

#[test]
fn a_colour_in_another_form_is_refused() {
    for text in ["#abc", "rgb(1, 2, 3)", "005fff", "#00GG00", "", "#"] {
        assert!(parse_hex(text).is_err(), "{text}");
    }
}

#[test]
fn a_colour_writes_back_as_it_was_read() {
    for text in ["#005fff", "#0b0c0e", "#ffffff0a", "#00000040"] {
        let colour = parse_hex(text).expect("colour");
        assert_eq!(hex(colour, true), text);
    }
}

#[test]
fn mixing_halfway_lands_between_two_colours() {
    let halfway = mix(from_bytes([0, 0, 0]), from_bytes([255, 255, 255]), 0.5);
    assert!(close(halfway.r, 0.5) && close(halfway.g, 0.5) && close(halfway.b, 0.5));
}

#[test]
fn a_translucent_colour_over_another_blends_by_its_alpha() {
    let top = Rgba { r: 1., g: 1., b: 1., a: 0.5 };
    let below = from_bytes([0, 0, 0]);
    let seen = over(top, below);
    assert!(close(seen.r, 0.5) && close(seen.a, 1.));
    assert!(close(over(below, top).r, 0.5));
}

#[test]
fn black_has_no_luminance_and_white_has_all_of_it() {
    assert!(close(luminance(from_bytes([0, 0, 0])), 0.));
    assert!(close(luminance(from_bytes([255, 255, 255])), 1.));
}
