use std::io::Write;

use crate::openlibrary::OpenLibrary;
use crate::paths::AppPaths;
use crate::{Error, Result};

/// The most pixels a cover may have. A small file can still decode to
/// gigabytes; refusing at download protects every app, not just the one
/// that checks when it draws (GTK's `load_cover` has the same cap).
pub const MAX_COVER_PIXELS: u64 = 40_000_000;

/// Downloads a cover into covers_dir as `<book_id>.<ext>`.
/// Returns the bare filename to store in `books.cover_path`, or None if the
/// image host had nothing.
pub fn download(
    ol: &OpenLibrary,
    paths: &AppPaths,
    book_id: &str,
    cover_url: &str,
) -> Result<Option<String>> {
    check_book_id(book_id)?;
    let Some((bytes, content_type)) = ol.fetch_image(cover_url)? else {
        return Ok(None);
    };
    store(paths, book_id, &bytes, &content_type).map(Some)
}

/// The id becomes a filename, so it must not be able to name a path.
fn check_book_id(book_id: &str) -> Result<()> {
    if book_id.is_empty()
        || !book_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::Invalid(format!("unexpected book id {book_id:?}")));
    }
    Ok(())
}

/// Saves downloaded bytes, once their header shows an image of a sane
/// size. They're written to `<name>.part` and renamed into place, so a
/// crash or full disk midway never leaves half a cover where the app looks.
fn store(paths: &AppPaths, book_id: &str, bytes: &[u8], content_type: &str) -> Result<String> {
    check_book_id(book_id)?;
    let size = imagesize::blob_size(bytes)
        .map_err(|_| Error::Invalid("the cover isn't an image we can read".into()))?;
    if (size.width as u64).saturating_mul(size.height as u64) > MAX_COVER_PIXELS {
        return Err(Error::Invalid(format!(
            "the cover is too large ({}×{})",
            size.width, size.height
        )));
    }
    let ext = match content_type.split(';').next().unwrap_or("").trim() {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    let filename = format!("{book_id}.{ext}");
    let target = paths.cover_file(&filename);
    let part = paths.cover_file(&format!("{filename}.part"));
    let written = std::fs::File::create(&part)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&part, &target));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e.into());
    }
    Ok(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Just the start of a PNG: the signature and the header chunk with
    /// the size, which is all a size check reads.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        out.extend(width.to_be_bytes());
        out.extend(height.to_be_bytes());
        out.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]); // depth, colour..., CRC
        out
    }

    fn gif(width: u16, height: u16) -> Vec<u8> {
        let mut out = b"GIF89a".to_vec();
        out.extend(width.to_le_bytes());
        out.extend(height.to_le_bytes());
        out.extend([0, 0, 0]);
        out
    }

    fn setup() -> (tempfile::TempDir, AppPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path()).unwrap();
        (dir, paths)
    }

    fn files_in(paths: &AppPaths) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&paths.covers_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_cover_is_saved_whole_under_its_book_id() {
        let (_dir, paths) = setup();
        let bytes = png(180, 270);
        let name = store(&paths, "abc-1", &bytes, "image/png").unwrap();
        assert_eq!(name, "abc-1.png");
        assert_eq!(std::fs::read(paths.cover_file(&name)).unwrap(), bytes);
        assert_eq!(files_in(&paths), ["abc-1.png"], "no .part left behind");

        // A new download replaces the old file.
        let again = gif(90, 135);
        let name = store(&paths, "abc-1", &again, "image/gif; x=y").unwrap();
        assert_eq!(name, "abc-1.gif");
        store(&paths, "abc-1", &bytes, "image/png").unwrap();
        assert_eq!(std::fs::read(paths.cover_file("abc-1.png")).unwrap(), bytes);
        assert_eq!(files_in(&paths), ["abc-1.gif", "abc-1.png"]);
    }

    #[test]
    fn oversized_covers_are_refused() {
        let (_dir, paths) = setup();
        // 40 megapixels exactly is fine; one row more is not.
        assert!(store(&paths, "a", &png(8_000, 5_000), "image/png").is_ok());
        let err = store(&paths, "b", &png(8_000, 5_001), "image/png").unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
        assert!(store(&paths, "c", &png(u32::MAX, u32::MAX), "image/png").is_err());
        assert!(store(&paths, "d", &gif(65_535, 65_535), "image/gif").is_err());
        assert_eq!(files_in(&paths), ["a.png"]);
    }

    #[test]
    fn only_images_are_kept() {
        let (_dir, paths) = setup();
        for bytes in [
            &b"<svg xmlns='http://www.w3.org/2000/svg'/>"[..],
            b"",
            b"\x89PN",
        ] {
            assert!(store(&paths, "a", bytes, "image/png").is_err());
        }
        assert!(files_in(&paths).is_empty());
        assert!(store(&paths, "../a", &png(1, 1), "image/png").is_err());
    }
}
