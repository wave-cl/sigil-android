//! The launcher icon: the mark of a key, at every density Android asks
//! for. By default the key of thirty-two 0x01 bytes: a mark like every
//! other in the app and nobody's in particular. Run through
//! `scripts/launcher-icon`.
//!
//!     launcher-icon [KEY] [RES_DIR]
//!
//! `KEY` is the key's text (base58); `RES_DIR` is `android/app/src/main/res`.

use std::path::PathBuf;

/// Base58 of thirty-two 0x01 bytes, as the app names a key.
const ALL_ONES: &str = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";

/// Launcher icon sizes, in pixels, by density bucket.
const DENSITIES: [(&str, usize); 5] = [
    ("mdpi", 48),
    ("hdpi", 72),
    ("xhdpi", 96),
    ("xxhdpi", 144),
    ("xxxhdpi", 192),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let key = args.next().unwrap_or_else(|| ALL_ONES.to_string());
    let res = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "android/app/src/main/res".to_string()),
    );
    for (bucket, size) in DENSITIES {
        let dir = res.join(format!("mipmap-{bucket}"));
        std::fs::create_dir_all(&dir).expect("the mipmap directory");
        // The plain icon, for a launcher that takes no adaptive one.
        write_png(
            &dir.join("ic_launcher.png"),
            size,
            &sigil_ui::identicon_raster(key.as_bytes(), size),
        );
        // The adaptive icon's layer: 108dp to the icon's 48, so the
        // launcher's mask -- the middle two thirds -- is the mark's circle,
        // and the mark fills the whole of it.
        let layer = size * 108 / 48;
        write_png(
            &dir.join("ic_launcher_layer.png"),
            layer,
            &sigil_ui::identicon_layer(key.as_bytes(), layer),
        );
    }
    let adaptive = res.join("mipmap-anydpi-v26");
    std::fs::create_dir_all(&adaptive).expect("the anydpi directory");
    let xml = adaptive.join("ic_launcher.xml");
    std::fs::write(
        &xml,
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <!-- Drawn by scripts/launcher-icon: the mark, as the whole of the icon. -->\n\
         <adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n\
         \x20   <background android:drawable=\"@mipmap/ic_launcher_layer\" />\n\
         \x20   <foreground android:drawable=\"@android:color/transparent\" />\n\
         </adaptive-icon>\n",
    )
    .expect("the adaptive icon");
    println!("{}", xml.display());
}

fn write_png(path: &std::path::Path, size: usize, pixels: &[u8]) {
    let file = std::fs::File::create(path).expect("the icon file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size as u32, size as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("the png header");
    writer.write_image_data(pixels).expect("the png data");
    writer.finish().expect("the png");
    println!("{} ({size}px)", path.display());
}
