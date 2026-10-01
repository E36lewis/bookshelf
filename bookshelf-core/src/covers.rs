use crate::openlibrary::OpenLibrary;
use crate::paths::AppPaths;
use crate::{Error, Result};

/// Downloads a cover into covers_dir as `<book_id>.<ext>`.
/// Returns the bare filename to store in `books.cover_path`, or None if the
/// image host had nothing.
pub fn download(
    ol: &OpenLibrary,
    paths: &AppPaths,
    book_id: &str,
    cover_url: &str,
) -> Result<Option<String>> {
    // The id becomes a filename, so it must not be able to name a path.
    if book_id.is_empty()
        || !book_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::Invalid(format!("unexpected book id {book_id:?}")));
    }
    let Some((bytes, content_type)) = ol.fetch_image(cover_url)? else {
        return Ok(None);
    };
    let ext = match content_type.split(';').next().unwrap_or("").trim() {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    let filename = format!("{book_id}.{ext}");
    std::fs::write(paths.cover_file(&filename), bytes)?;
    Ok(Some(filename))
}
