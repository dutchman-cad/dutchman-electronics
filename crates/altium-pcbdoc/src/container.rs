//! Tolerance for one known flaw in OLE / Compound File Binary containers written by other
//! programs (Microsoft's public MS-CFB specification describes the layout used here).
//!
//! A container's file allocation table (FAT) has one 4-byte entry per sector. Entries for
//! sectors past the end of the file must say "free" (`0xFFFFFFFF`). Some writers fill them
//! with "end of chain" (`0xFFFFFFFE`) instead, and the `cfb` crate then refuses the file
//! ("FAT has 128 entries, but file has only 22 sectors"). Those entries point at sectors that
//! do not exist, so treating them as free changes nothing that can be read.

use std::borrow::Cow;

const FREE: u32 = 0xFFFF_FFFF;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
/// FAT sector numbers held in the header itself; more than this needs the DIFAT chain, which a
/// small file never has, so such files are left alone.
const HEADER_DIFAT: usize = 109;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// The container's bytes, with "end of chain" FAT entries for sectors past the end of the file
/// set to "free". Returns the bytes unchanged (borrowed) when nothing needed changing, and how
/// many entries were changed.
pub(crate) fn normalized(bytes: &[u8]) -> (Cow<'_, [u8]>, usize) {
    match fixes(bytes) {
        Some(offsets) if !offsets.is_empty() => {
            let mut owned = bytes.to_vec();
            for at in &offsets {
                if let Some(slot) = owned.get_mut(*at..at.saturating_add(4)) {
                    slot.copy_from_slice(&FREE.to_le_bytes());
                }
            }
            (Cow::Owned(owned), offsets.len())
        }
        _ => (Cow::Borrowed(bytes), 0),
    }
}

/// Byte offsets of the FAT entries to set free, or `None` when the header cannot be read.
fn fixes(bytes: &[u8]) -> Option<Vec<usize>> {
    let shift = u16_at(bytes, 30)?;
    if !(9..=12).contains(&shift) {
        return None;
    }
    let sector = 1usize << shift;
    let fat_sectors = usize::try_from(u32_at(bytes, 44)?).ok()?;
    if fat_sectors > HEADER_DIFAT {
        return None;
    }
    // The header fills the first sector (512 bytes, or 4096 in version 4 files), so sector `n`
    // starts at `(n + 1) * sector`. A last sector may be cut short; it still exists.
    let file_sectors = bytes.len().div_ceil(sector).saturating_sub(1);
    let per_sector = sector / 4;
    let mut out = Vec::new();
    for k in 0..fat_sectors {
        let fat = usize::try_from(u32_at(bytes, 76usize.checked_add(k.checked_mul(4)?)?)?).ok()?;
        let start = fat.checked_add(1)?.checked_mul(sector)?;
        for i in 0..per_sector {
            let index = k.checked_mul(per_sector)?.checked_add(i)?;
            let at = start.checked_add(i.checked_mul(4)?)?;
            if index >= file_sectors && u32_at(bytes, at)? == END_OF_CHAIN {
                out.push(at);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    /// A real container from the `cfb` crate, then its spare FAT entries spoiled the way some
    /// writers spoil them.
    fn spoiled() -> (Vec<u8>, Vec<u8>) {
        let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        cf.create_stream("/Data")
            .unwrap()
            .write_all(b"hello")
            .unwrap();
        cf.flush().unwrap();
        let good = cf.into_inner().into_inner();
        let mut bad = good.clone();
        let sector = 1usize << u16_at(&bad, 30).unwrap();
        let fat = u32_at(&bad, 76).unwrap() as usize;
        let start = (fat + 1) * sector;
        let file_sectors = bad.len().div_ceil(sector) - 1;
        for i in file_sectors..sector / 4 {
            let at = start + i * 4;
            bad[at..at + 4].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        }
        (good, bad)
    }

    #[test]
    fn spoiled_spare_entries_are_set_free() {
        let (good, bad) = spoiled();
        assert!(cfb::CompoundFile::open(Cursor::new(&bad[..])).is_err());
        let (fixed, changed) = normalized(&bad);
        assert!(changed > 0);
        assert_eq!(fixed.as_ref(), &good[..]);
        let mut cf = cfb::CompoundFile::open(Cursor::new(fixed.as_ref())).unwrap();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut cf.open_stream("/Data").unwrap(), &mut text).unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn a_good_file_is_left_alone() {
        let (good, _) = spoiled();
        let (same, changed) = normalized(&good);
        assert_eq!(changed, 0);
        assert!(matches!(same, Cow::Borrowed(_)));
    }

    #[test]
    fn garbage_is_left_alone() {
        assert_eq!(normalized(b"short").1, 0);
        assert_eq!(normalized(&[0xff; 600]).1, 0);
    }
}
