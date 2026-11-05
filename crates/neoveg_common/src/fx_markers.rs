//! The markers of a video effect chain (manual v18, *Pan/Crop* and *Composite* buttons of the plug-in
//! chain): a place in the chain that is not an effect but says when something else happens.
//!
//! * [`PAN_CROP`] in the chain of an event: the effects before it run on the picture of the event
//!   before its Pan/Crop (so a blur is not stretched by a zoom), the effects after it run after.
//!   Without the marker every effect runs after the Pan/Crop, as it always did.
//! * [`COMPOSITE`] in the chain of a track: the effects before it run on the layer of the track
//!   before it is composited with the tracks below, the effects after it run on the result of the
//!   compositing (the layer and everything under it). Without the marker every effect runs before.
//!
//! A marker is stored in the chain as an entry with one of these ids, no parameters and no bypass.

/// Where the Pan/Crop of an event happens in its chain.
pub const PAN_CROP: &str = "marker_pan_crop";
/// Where the compositing of a track happens in its chain.
pub const COMPOSITE: &str = "marker_composite";

/// Whether `kind` is the id of a marker (and not of an effect).
pub fn is_marker(kind: &str) -> bool {
    kind == PAN_CROP || kind == COMPOSITE
}

/// Name of a marker in the panel (a key of the translation files).
pub fn label(kind: &str) -> &'static str {
    match kind {
        PAN_CROP => "Marcador Pan/Crop",
        COMPOSITE => "Marcador Composite",
        _ => "",
    }
}

/// What a marker says, for the panel.
pub fn hint(kind: &str) -> &'static str {
    match kind {
        PAN_CROP => "Os efeitos acima rodam antes do Pan/Crop do evento; os abaixo, depois.",
        COMPOSITE => {
            "Os efeitos acima rodam na camada da faixa; os abaixo, no resultado da composição."
        }
        _ => "",
    }
}

/// Splits `chain` at the first entry for which `is_marker` is true: what comes before it, and what
/// comes after it (`None` when there is no marker, so the whole chain is one part).
pub fn split<'a, T>(chain: &'a [T], is_marker: impl Fn(&T) -> bool) -> (&'a [T], Option<&'a [T]>) {
    match chain.iter().position(is_marker) {
        Some(at) => (&chain[..at], Some(&chain[at + 1..])),
        None => (chain, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_marker_ids_are_markers_and_each_has_a_name_and_a_hint() {
        assert!(is_marker(PAN_CROP) && is_marker(COMPOSITE));
        assert!(!is_marker("gaussian_blur") && !is_marker(""));
        for kind in [PAN_CROP, COMPOSITE] {
            assert!(!label(kind).is_empty() && !hint(kind).is_empty());
        }
        assert_eq!(label("gaussian_blur"), "");
    }

    #[test]
    fn a_chain_is_split_at_its_marker_and_a_chain_without_one_is_whole() {
        let chain = ["blur", "marker", "sharpen", "invert"];
        let (before, after) = split(&chain, |k| *k == "marker");
        assert_eq!(before, ["blur"]);
        assert_eq!(after, Some(&["sharpen", "invert"][..]));
        let (all, none) = split(&chain[2..], |k| *k == "marker");
        assert_eq!(all, ["sharpen", "invert"]);
        assert!(none.is_none());
        // The marker at an end leaves an empty side
        let (before, after) = split(&["marker", "x"], |k| *k == "marker");
        assert!(before.is_empty() && after == Some(&["x"][..]));
        let (before, after) = split(&["x", "marker"], |k| *k == "marker");
        assert_eq!(before, ["x"]);
        assert_eq!(after, Some(&[][..]));
    }
}
