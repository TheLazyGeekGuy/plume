//! `cargo xtask icns <out.icns> <dir.iconset>`: pack an Apple iconset folder into a macOS `.icns`
//! without `iconutil`, so `packaging/icons.sh` can write every icon on any OS.
//!
//! An iconset holds `icon_<n>x<n>.png` and `icon_<n>x<n>@2x.png` for n in 16, 32, 128, 256 and
//! 512. Each PNG is stored as is under the OSType `iconutil` uses for it (PNG payloads are read by
//! macOS 10.7 and later). Missing sizes are skipped.

use std::path::Path;

/// Iconset file names and their ICNS element types, in `iconutil`'s order.
pub const ELEMENTS: &[(&str, &[u8; 4], u32)] = &[
    ("icon_16x16.png", b"icp4", 16),
    ("icon_16x16@2x.png", b"ic11", 32),
    ("icon_32x32.png", b"icp5", 32),
    ("icon_32x32@2x.png", b"ic12", 64),
    ("icon_128x128.png", b"ic07", 128),
    ("icon_128x128@2x.png", b"ic13", 256),
    ("icon_256x256.png", b"ic08", 256),
    ("icon_256x256@2x.png", b"ic14", 512),
    ("icon_512x512.png", b"ic09", 512),
    ("icon_512x512@2x.png", b"ic10", 1024),
];

/// An ICNS file holding `entries` (element type, PNG bytes). Each PNG must have the pixel size
/// its type stands for.
pub fn pack(entries: &[(&[u8; 4], Vec<u8>)]) -> Result<Vec<u8>, String> {
    if entries.is_empty() {
        return Err("need at least one PNG".into());
    }
    let mut body = Vec::new();
    for (kind, png) in entries {
        let (w, h) = crate::ico::png_size(png)?;
        let want = ELEMENTS.iter().find(|e| e.1 == *kind).map(|e| e.2).ok_or_else(|| "unknown icns element type".to_string())?;
        if w != want || h != want {
            return Err(format!("{} needs a {want}x{want} PNG, got {w}x{h}", String::from_utf8_lossy(&kind[..])));
        }
        let len = u32::try_from(png.len() + 8).map_err(|_| "PNG too large".to_string())?;
        body.extend_from_slice(&kind[..]);
        body.extend_from_slice(&len.to_be_bytes());
        body.extend_from_slice(png);
    }
    let total = u32::try_from(body.len() + 8).map_err(|_| "icns too large".to_string())?;
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn run(args: &[&str]) -> Result<(), String> {
    let [out, dir] = args else {
        return Err("usage: cargo xtask icns <out.icns> <dir.iconset>".into());
    };
    let dir = Path::new(dir);
    let mut entries = Vec::new();
    for (name, kind, _) in ELEMENTS {
        let path = dir.join(name);
        if path.exists() {
            entries.push((*kind, std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?));
        }
    }
    let icns = pack(&entries)?;
    std::fs::write(Path::new(out), &icns).map_err(|e| format!("write {out}: {e}"))?;
    println!("{out}: {} images, {} bytes", entries.len(), icns.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_png(size: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        v.extend_from_slice(&size.to_be_bytes());
        v.extend_from_slice(&size.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 1, 2, 3, 4]);
        v
    }

    #[test]
    fn packs_header_and_elements() {
        let a = fake_png(16);
        let b = fake_png(1024);
        let icns = pack(&[(b"icp4", a.clone()), (b"ic10", b.clone())]).unwrap();
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize, icns.len());
        assert_eq!(&icns[8..12], b"icp4");
        assert_eq!(u32::from_be_bytes(icns[12..16].try_into().unwrap()) as usize, a.len() + 8);
        assert_eq!(&icns[16..16 + a.len()], &a[..]);
        assert_eq!(&icns[16 + a.len()..20 + a.len()], b"ic10");
    }

    #[test]
    fn rejects_wrong_sizes() {
        assert!(pack(&[]).is_err());
        assert!(pack(&[(b"ic07", fake_png(64))]).is_err());
        assert!(pack(&[(b"zzzz", fake_png(16))]).is_err());
    }

    #[test]
    fn committed_icon_is_valid() {
        let path = crate::root().join("assets/app-icon/plume.icns");
        let Ok(icns) = std::fs::read(&path) else { return };
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize, icns.len());
    }
}
