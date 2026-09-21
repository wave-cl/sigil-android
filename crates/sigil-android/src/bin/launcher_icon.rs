//! The launcher icon: the mark of a key, at every density Android asks
//! for. By default the key of thirty-two 0x01 bytes: a mark like every
//! other in the app and nobody's in particular. Run through
//! `scripts/launcher-icon`.
//!
//!     launcher-icon [KEY] [RES_DIR] [PLAY_DIR]
//!
//! `KEY` is the key's text (base58); `RES_DIR` is `android/app/src/main/res`;
//! `PLAY_DIR` is `android/play`, where the store listing's graphics go: the
//! 512-pixel icon and the 1024×500 feature graphic Play insists on, both the
//! same mark over its own background colour.

use std::path::PathBuf;

/// Base58 of thirty-two 0x01 bytes, as the app names a key. From
/// `sigil-ui`, which draws the same mark on the phone's welcome screen: two
/// copies of a constant that has to match a committed picture is one too
/// many.
use sigil_ui::MARK_KEY as ALL_ONES;

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
    let play = PathBuf::from(args.next().unwrap_or_else(|| "android/play".to_string()));
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

    // The store listing. Play rounds the icon's corners itself and wants the
    // art to fill the square, so the mark sits at nine tenths, not the
    // launcher's two thirds; the feature graphic is the same mark centred
    // on a field of its background.
    std::fs::create_dir_all(&play).expect("the play directory");
    write_png_wh(
        &play.join("icon-512.png"),
        512,
        512,
        &mark_on_field(key.as_bytes(), 512, 512, 512 * 9 / 10),
    );
    write_png_wh(
        &play.join("feature-1024x500.png"),
        1024,
        500,
        &mark_on_field(key.as_bytes(), 1024, 500, 400),
    );
}

/// A `width`×`height` field of the key's background colour with its mark,
/// `mark` pixels across, in the middle.
fn mark_on_field(id: &[u8], width: usize, height: usize, mark: usize) -> Vec<u8> {
    let (back, _) = sigil_ui::identicon::colours(id);
    let raster = sigil_ui::identicon_raster(id, mark);
    let (ox, oy) = ((width - mark) / 2, (height - mark) / 2);
    let mut out = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let inside = x >= ox && y >= oy && x - ox < mark && y - oy < mark;
            let px = if inside {
                let i = ((y - oy) * mark + (x - ox)) * 4;
                let p = &raster[i..i + 4];
                // Over the background, so the circle's soft edge blends
                // into the field rather than into transparency.
                let a = p[3] as u32;
                [
                    ((p[0] as u32 * a + back.r() as u32 * (255 - a)) / 255) as u8,
                    ((p[1] as u32 * a + back.g() as u32 * (255 - a)) / 255) as u8,
                    ((p[2] as u32 * a + back.b() as u32 * (255 - a)) / 255) as u8,
                    255,
                ]
            } else {
                [back.r(), back.g(), back.b(), 255]
            };
            out.extend_from_slice(&px);
        }
    }
    out
}

fn write_png(path: &std::path::Path, size: usize, pixels: &[u8]) {
    write_png_wh(path, size, size, pixels);
}

fn write_png_wh(path: &std::path::Path, width: usize, height: usize, pixels: &[u8]) {
    let file = std::fs::File::create(path).expect("the icon file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("the png header");
    writer.write_image_data(pixels).expect("the png data");
    writer.finish().expect("the png");
    println!("{} ({width}×{height}px)", path.display());
}
