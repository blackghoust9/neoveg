//! Clean-room license guard for `neoveg_video`: enforces shared dynamic linking (LGPL v2.1+)
//! whenever the optional `ffmpeg` feature is enabled, refusing static FFmpeg builds.

fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_STATIC");
    println!("cargo:rerun-if-env-changed=FFMPEG_LIBS_DIR");
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    let static_env = std::env::var("FFMPEG_STATIC")
        .ok()
        .is_some_and(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "YES"));

    assert!(
        !static_env,
        "Clean-room / LGPLv2.1+ compliance error: FFMPEG_STATIC is enabled. \
         Neoveg requires shared dynamic linking (.dll / .so / .dylib) for FFmpeg."
    );
}
