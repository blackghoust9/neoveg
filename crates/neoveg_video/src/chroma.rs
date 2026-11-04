//! Applies [`ChromaKey`] (green/blue screen matte + luma-preserving spill suppression) to an RGBA
//! [`FrameBuffer`].

use neoveg_common::chroma::ChromaKey;

use crate::frame::FrameBuffer;

/// Keys out the configured screen color on `frame`. A disabled [`ChromaKey`] is a zero-cost no-op.
pub fn apply_chroma_key(frame: &mut FrameBuffer, key: &ChromaKey) {
    if key.is_disabled() {
        return;
    }
    let key = key.sanitized();
    let mut last_in = [u8::MAX; 4];
    let mut last_out = [0_u8; 4];
    let mut have_last = false;

    for px in frame.rgba.as_chunks_mut::<4>().0 {
        let src = *px;
        if !(have_last && src == last_in) {
            let out = key.apply_rgba([
                f32::from(src[0]) / 255.0,
                f32::from(src[1]) / 255.0,
                f32::from(src[2]) / 255.0,
                f32::from(src[3]) / 255.0,
            ]);
            last_out = [
                (out[0] * 255.0).round() as u8,
                (out[1] * 255.0).round() as u8,
                (out[2] * 255.0).round() as u8,
                (out[3] * 255.0).round() as u8,
            ];
            last_in = src;
            have_last = true;
        }
        *px = last_out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::smpte_color_bars;

    #[test]
    fn disabled_key_leaves_frame_untouched_and_enabled_key_punches_out_green_bar() {
        let orig = smpte_color_bars(70, 8);
        let mut untouched = orig.clone();
        apply_chroma_key(&mut untouched, &ChromaKey::default());
        assert_eq!(untouched, orig);

        let mut keyed = orig.clone();
        apply_chroma_key(
            &mut keyed,
            &ChromaKey {
                enabled: true,
                key_color: [0.0, 0.75, 0.0],
                similarity: 0.22,
                smoothness: 0.06,
                spill: 0.6,
                show_matte: false,
            },
        );
        // Bar index 3 of 7 (x ~ 35) in SMPTE 75% color bars is pure green [0, 191, 0, 255];
        // bar index 4 (x ~ 45) is magenta [191, 0, 191, 255].
        let green_px = keyed.pixel(35, 2);
        let magenta_px = keyed.pixel(45, 2);
        assert_eq!(
            green_px[3], 0,
            "SMPTE green bar should be keyed out: {green_px:?}"
        );
        assert_eq!(
            magenta_px[3], 255,
            "SMPTE magenta bar should stay opaque: {magenta_px:?}"
        );
    }
}
