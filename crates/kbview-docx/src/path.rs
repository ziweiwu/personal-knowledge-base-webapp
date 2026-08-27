//! Package-relative path arithmetic and media-type guessing.

/// Resolve an OOXML relationship target against the directory of the part
/// that declared it, yielding a path from the package root.
pub(crate) fn resolve_relative(base_dir: &str, target: &str) -> String {
    let (base, relative) = match target.strip_prefix('/') {
        Some(from_root) => ("", from_root),
        None => (base_dir, target),
    };

    let mut segments: Vec<&str> = Vec::new();
    for segment in base.split('/').chain(relative.split('/')) {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// The directory containing `part_path`, without a trailing slash.
pub(crate) fn parent_dir(part_path: &str) -> &str {
    match part_path.rfind('/') {
        Some(index) => &part_path[..index],
        None => "",
    }
}

/// OOXML keeps a part's relationships in a sibling `_rels` directory, in a
/// file named after the part itself.
pub(crate) fn rels_path_for(part_path: &str) -> String {
    let directory = parent_dir(part_path);
    let file_name = &part_path[if directory.is_empty() {
        0
    } else {
        directory.len() + 1
    }..];
    if directory.is_empty() {
        format!("_rels/{file_name}.rels")
    } else {
        format!("{directory}/_rels/{file_name}.rels")
    }
}

/// Guess a media type from a part's file extension.
pub(crate) fn mime_for_path(part_path: &str) -> &'static str {
    let extension = part_path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "emf" => "image/emf",
        "wmf" => "image/wmf",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_targets_relative_to_the_owning_part() {
        assert_eq!(
            resolve_relative("word", "media/image1.png"),
            "word/media/image1.png"
        );
        assert_eq!(
            resolve_relative("word", "../customXml/item1.xml"),
            "customXml/item1.xml"
        );
        assert_eq!(
            resolve_relative("word", "/word/media/a.png"),
            "word/media/a.png"
        );
        assert_eq!(resolve_relative("", "document.xml"), "document.xml");
    }

    #[test]
    fn builds_rels_paths() {
        assert_eq!(
            rels_path_for("word/document.xml"),
            "word/_rels/document.xml.rels"
        );
        assert_eq!(rels_path_for("document.xml"), "_rels/document.xml.rels");
    }

    #[test]
    fn guesses_media_types() {
        assert_eq!(mime_for_path("word/media/image1.PNG"), "image/png");
        assert_eq!(mime_for_path("word/media/image2.jpeg"), "image/jpeg");
        assert_eq!(
            mime_for_path("word/media/thing"),
            "application/octet-stream"
        );
    }
}
