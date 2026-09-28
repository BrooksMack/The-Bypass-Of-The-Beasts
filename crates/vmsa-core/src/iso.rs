//! Minimal ISO-9660 reader used to identify Windows installation media from its own
//! metadata rather than its file name: the volume label (e.g. `CCCOMA_A64FRE_EN-US_DV9`),
//! the EFI boot loader present (`BOOTX64.EFI` vs `BOOTAA64.EFI`) and `sources\install.wim|esd`.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::profile::GuestArch;
use crate::{CoreError, Result};

const SECTOR: u64 = 2048;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IsoInfo {
    pub path: String,
    pub size_bytes: u64,
    pub volume_id: String,
    pub has_efi_x64_boot: bool,
    pub has_efi_arm64_boot: bool,
    pub has_install_image: bool,
    /// Architecture inferred from the EFI boot loader present.
    pub arch: Option<GuestArch>,
    pub looks_like_windows: bool,
    pub language_hint: Option<String>,
}

impl IsoInfo {
    pub fn suitable_for(&self, guest: GuestArch) -> std::result::Result<(), String> {
        if !self.looks_like_windows {
            return Err("This file does not look like Windows installation media (no sources\\install.wim or install.esd found).".into());
        }
        match (guest, self.arch) {
            (GuestArch::X64, Some(GuestArch::X64)) | (GuestArch::Arm64, Some(GuestArch::Arm64)) => {
                Ok(())
            }
            (GuestArch::Arm64, Some(GuestArch::X64)) => Err(
                "This is a Windows x64 ISO, but this computer needs the Windows 11 ARM64 ISO."
                    .into(),
            ),
            (GuestArch::X64, Some(GuestArch::Arm64)) => Err(
                "This is a Windows ARM64 ISO, but this computer needs the Windows 11 x64 ISO."
                    .into(),
            ),
            (_, None) => Err(
                "Could not find an EFI boot loader on this ISO, so it cannot boot a Windows 11 VM."
                    .into(),
            ),
        }
    }
}

/// Inspect an ISO file. Reads only the primary volume descriptor and a few directory records.
pub fn inspect(path: &Path) -> Result<IsoInfo> {
    let mut f = std::fs::File::open(path).map_err(|e| CoreError::io(path, e))?;
    let size_bytes = f.metadata().map_err(|e| CoreError::io(path, e))?.len();
    inspect_reader(&mut f, size_bytes, &path.display().to_string())
}

pub fn inspect_reader<R: Read + Seek>(r: &mut R, size_bytes: u64, path: &str) -> Result<IsoInfo> {
    // Volume descriptors start at sector 16. Find the primary (type 1) descriptor.
    let mut pvd = None;
    let mut joliet_root: Option<(u64, u64)> = None;
    for i in 16..48u64 {
        let mut buf = [0u8; 2048];
        r.seek(SeekFrom::Start(i * SECTOR))
            .map_err(|e| CoreError::io(path, e))?;
        if r.read_exact(&mut buf).is_err() {
            break;
        }
        if &buf[1..6] != b"CD001" {
            break;
        }
        match buf[0] {
            1 if pvd.is_none() => pvd = Some(buf),
            2 => {
                // Supplementary (Joliet) volume descriptor: same layout, UCS-2 names.
                let (extent, len) = root_record(&buf[156..190]);
                joliet_root = Some((extent, len));
            }
            255 => break,
            _ => {}
        }
    }
    let pvd = pvd.ok_or_else(|| {
        CoreError::InvalidInput("Not an ISO-9660 image (no primary volume descriptor).".into())
    })?;
    let volume_id = String::from_utf8_lossy(&pvd[40..72]).trim().to_string();
    let (root_extent, root_len) = root_record(&pvd[156..190]);

    // Prefer the Joliet tree (mixed-case names) and fall back to the plain ISO tree.
    let mut probe = |names: &[&str]| -> bool {
        if let Some((ext, len)) = joliet_root {
            if let Ok(true) = path_exists(r, ext, len, names, true) {
                return true;
            }
        }
        matches!(
            path_exists(r, root_extent, root_len, names, false),
            Ok(true)
        )
    };

    let has_efi_x64_boot = probe(&["EFI", "BOOT", "BOOTX64.EFI"]);
    let has_efi_arm64_boot = probe(&["EFI", "BOOT", "BOOTAA64.EFI"]);
    let has_install_image =
        probe(&["SOURCES", "INSTALL.WIM"]) || probe(&["SOURCES", "INSTALL.ESD"]);

    let arch = match (has_efi_x64_boot, has_efi_arm64_boot) {
        (true, false) => Some(GuestArch::X64),
        (false, true) => Some(GuestArch::Arm64),
        _ => None,
    };
    let language_hint = language_from_volume_id(&volume_id);
    Ok(IsoInfo {
        path: path.to_string(),
        size_bytes,
        volume_id: volume_id.clone(),
        has_efi_x64_boot,
        has_efi_arm64_boot,
        has_install_image,
        arch,
        looks_like_windows: has_install_image,
        language_hint,
    })
}

/// Microsoft volume labels look like `CCCOMA_X64FRE_EN-US_DV9`; pull out the `EN-US` part.
pub fn language_from_volume_id(volume_id: &str) -> Option<String> {
    volume_id
        .split('_')
        .find(|p| p.len() == 5 && p.as_bytes()[2] == b'-')
        .map(|s| s.to_string())
}

fn root_record(rec: &[u8]) -> (u64, u64) {
    let extent = u32::from_le_bytes([rec[2], rec[3], rec[4], rec[5]]) as u64;
    let len = u32::from_le_bytes([rec[10], rec[11], rec[12], rec[13]]) as u64;
    (extent, len)
}

/// Walk a directory chain looking for `names` (case-insensitive). Returns Ok(true) if found.
fn path_exists<R: Read + Seek>(
    r: &mut R,
    mut extent: u64,
    mut len: u64,
    names: &[&str],
    joliet: bool,
) -> Result<bool> {
    for (i, want) in names.iter().enumerate() {
        let last = i == names.len() - 1;
        let mut found = None;
        let mut data = vec![0u8; len.min(64 * SECTOR) as usize];
        r.seek(SeekFrom::Start(extent * SECTOR))
            .map_err(|e| CoreError::Io {
                path: String::new(),
                message: e.to_string(),
            })?;
        let n = r.read(&mut data).map_err(|e| CoreError::Io {
            path: String::new(),
            message: e.to_string(),
        })?;
        data.truncate(n);
        let mut off = 0usize;
        while off + 33 <= data.len() {
            let rec_len = data[off] as usize;
            if rec_len == 0 {
                // Records never cross sector boundaries; skip padding.
                off = (off / SECTOR as usize + 1) * SECTOR as usize;
                continue;
            }
            if off + rec_len > data.len() {
                break;
            }
            let rec = &data[off..off + rec_len];
            let name_len = rec[32] as usize;
            let flags = rec[25];
            let raw_name = &rec[33..(33 + name_len).min(rec.len())];
            let name = if joliet {
                let units: Vec<u16> = raw_name
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&units)
            } else {
                String::from_utf8_lossy(raw_name).into_owned()
            };
            let name = name.split(';').next().unwrap_or("").to_string();
            if name.eq_ignore_ascii_case(want) {
                let is_dir = flags & 2 != 0;
                if last || is_dir {
                    found = Some((root_record(rec), is_dir));
                    break;
                }
            }
            off += rec_len;
        }
        match found {
            Some(((e, l), is_dir)) => {
                if last {
                    return Ok(true);
                }
                if !is_dir {
                    return Ok(false);
                }
                extent = e;
                len = l;
            }
            None => return Ok(false),
        }
    }
    Ok(false)
}

#[cfg(test)]
pub mod testutil {
    //! Builds tiny synthetic ISO-9660 images for tests (plain ISO tree only, no Joliet).
    use super::SECTOR;

    pub struct Entry {
        pub name: &'static str,
        pub children: Vec<Entry>,
    }

    pub fn dir(name: &'static str, children: Vec<Entry>) -> Entry {
        Entry { name, children }
    }
    pub fn file(name: &'static str) -> Entry {
        Entry {
            name,
            children: vec![],
        }
    }

    fn record(name: &str, extent: u32, len: u32, is_dir: bool) -> Vec<u8> {
        let id: Vec<u8> = if is_dir {
            name.as_bytes().to_vec()
        } else {
            format!("{name};1").into_bytes()
        };
        let mut rec = vec![0u8; 33 + id.len()];
        if rec.len() % 2 == 1 {
            rec.push(0);
        }
        rec[0] = rec.len() as u8;
        rec[2..6].copy_from_slice(&extent.to_le_bytes());
        rec[6..10].copy_from_slice(&extent.to_be_bytes());
        rec[10..14].copy_from_slice(&len.to_le_bytes());
        rec[14..18].copy_from_slice(&len.to_be_bytes());
        rec[25] = if is_dir { 2 } else { 0 };
        rec[32] = id.len() as u8;
        rec[33..33 + id.len()].copy_from_slice(&id);
        rec
    }

    /// Lay out directories sector by sector, starting at sector 18.
    fn layout(entry: &Entry, next: &mut u32, sectors: &mut Vec<(u32, Vec<u8>)>) -> u32 {
        let my = *next;
        *next += 1;
        let mut body = Vec::new();
        body.extend(record("\0", my, 2048, true));
        body.extend(record("\u{1}", my, 2048, true));
        let mut children = Vec::new();
        for c in &entry.children {
            if c.children.is_empty() {
                let ext = *next;
                *next += 1;
                sectors.push((ext, vec![0u8; 2048]));
                children.push((c.name, ext, false));
            } else {
                let ext = layout(c, next, sectors);
                children.push((c.name, ext, true));
            }
        }
        for (n, e, d) in children {
            body.extend(record(n, e, 2048, d));
        }
        body.resize(2048, 0);
        sectors.push((my, body));
        my
    }

    pub fn build_iso(volume_id: &str, root: Entry) -> Vec<u8> {
        let mut sectors = Vec::new();
        let mut next = 18u32;
        let root_extent = layout(&root, &mut next, &mut sectors);
        let total = next as usize;
        let mut img = vec![0u8; total * SECTOR as usize];
        // PVD at sector 16
        let pvd = &mut img[16 * 2048..17 * 2048];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        let vid = format!("{volume_id:<32}");
        pvd[40..72].copy_from_slice(&vid.as_bytes()[..32]);
        let rr = record("\0", root_extent, 2048, true);
        pvd[156..156 + rr.len()].copy_from_slice(&rr);
        // terminator at 17
        img[17 * 2048] = 255;
        img[17 * 2048 + 1..17 * 2048 + 6].copy_from_slice(b"CD001");
        for (ext, data) in sectors {
            let o = ext as usize * 2048;
            img[o..o + 2048].copy_from_slice(&data);
        }
        img
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;
    use std::io::Cursor;

    #[test]
    fn detects_arm64_windows_iso() {
        let img = build_iso(
            "CCCOMA_A64FRE_EN-US_DV9",
            dir(
                "",
                vec![
                    dir("EFI", vec![dir("BOOT", vec![file("BOOTAA64.EFI")])]),
                    dir("SOURCES", vec![file("INSTALL.WIM")]),
                ],
            ),
        );
        let info = inspect_reader(&mut Cursor::new(&img), img.len() as u64, "test.iso").unwrap();
        assert_eq!(info.volume_id, "CCCOMA_A64FRE_EN-US_DV9");
        assert_eq!(info.arch, Some(GuestArch::Arm64));
        assert!(info.looks_like_windows);
        assert_eq!(info.language_hint.as_deref(), Some("EN-US"));
        assert!(info.suitable_for(GuestArch::Arm64).is_ok());
        assert!(info
            .suitable_for(GuestArch::X64)
            .unwrap_err()
            .contains("ARM64 ISO"));
    }

    #[test]
    fn detects_x64_windows_iso_with_esd() {
        let img = build_iso(
            "CCCOMA_X64FRE_DE-DE_DV9",
            dir(
                "",
                vec![
                    dir("EFI", vec![dir("BOOT", vec![file("BOOTX64.EFI")])]),
                    dir("SOURCES", vec![file("INSTALL.ESD")]),
                ],
            ),
        );
        let info = inspect_reader(&mut Cursor::new(&img), img.len() as u64, "x.iso").unwrap();
        assert_eq!(info.arch, Some(GuestArch::X64));
        assert!(info.suitable_for(GuestArch::X64).is_ok());
        assert!(info
            .suitable_for(GuestArch::Arm64)
            .unwrap_err()
            .contains("x64 ISO"));
    }

    #[test]
    fn rejects_non_windows_iso() {
        let img = build_iso(
            "UBUNTU",
            dir(
                "",
                vec![
                    dir("EFI", vec![dir("BOOT", vec![file("BOOTX64.EFI")])]),
                    dir("CASPER", vec![file("VMLINUZ")]),
                ],
            ),
        );
        let info = inspect_reader(&mut Cursor::new(&img), img.len() as u64, "u.iso").unwrap();
        assert!(!info.looks_like_windows);
        assert!(info.suitable_for(GuestArch::X64).is_err());
    }

    #[test]
    fn rejects_garbage() {
        let img = vec![0u8; 40 * 2048];
        assert!(inspect_reader(&mut Cursor::new(&img), img.len() as u64, "g.iso").is_err());
    }
}
