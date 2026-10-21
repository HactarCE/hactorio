use std::fs;
use std::io::Write;
use std::path::PathBuf;

use hactorio_data::IconSource;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hactorio-icons-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reads_base_icons() {
    let dir = temp_dir("base");
    let icons = dir.join("base/graphics/icons");
    fs::create_dir_all(&icons).unwrap();
    fs::write(icons.join("iron-plate.png"), b"png-bytes").unwrap();

    let source = IconSource::new(&dir, dir.join("mods"));
    assert_eq!(
        source
            .read("__base__/graphics/icons/iron-plate.png")
            .as_deref(),
        Some(&b"png-bytes"[..])
    );
    assert!(
        source
            .read("__base__/graphics/icons/missing.png")
            .is_none()
    );
    assert!(source.read("not-a-path").is_none());

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reads_mod_icons_from_directories_and_zips() {
    let dir = temp_dir("mods");
    let mods = dir.join("mods");
    fs::create_dir_all(&mods).unwrap();

    let unpacked = mods.join("mymod_1.0.0/graphics/icons");
    fs::create_dir_all(&unpacked).unwrap();
    fs::write(unpacked.join("a.png"), b"a").unwrap();

    let zip_path = mods.join("zipmod_2.0.0.zip");
    let file = fs::File::create(&zip_path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file(
            "zipmod_2.0.0/graphics/icons/b.png",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(b"b").unwrap();
    writer.finish().unwrap();

    let source = IconSource::new(dir.join("data"), &mods);
    assert_eq!(
        source.read("__mymod__/graphics/icons/a.png").as_deref(),
        Some(&b"a"[..])
    );
    assert_eq!(
        source.read("__zipmod__/graphics/icons/b.png").as_deref(),
        Some(&b"b"[..])
    );
    assert!(
        source
            .read("__othermod__/graphics/icons/a.png")
            .is_none()
    );

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn decodes_png_icons() {
    let dir = temp_dir("decode");
    let icons = dir.join("base/graphics/icons");
    fs::create_dir_all(&icons).unwrap();
    image::RgbaImage::from_pixel(2, 3, image::Rgba([255, 0, 0, 255]))
        .save(icons.join("red.png"))
        .unwrap();

    let source = IconSource::new(&dir, dir.join("mods"));
    let decoded = source.read_rgba("__base__/graphics/icons/red.png").unwrap();

    assert_eq!((decoded.width, decoded.height), (2, 3));
    assert_eq!(&decoded.rgba[..4], &[255, 0, 0, 255]);

    fs::remove_dir_all(dir).unwrap();
}
