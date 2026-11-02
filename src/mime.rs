// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::Path; // Filesystem path type for representing file and directory paths.

// #############################################################################################
// MIME type function
// Returns the appropriate MIME type for a file based on its extension.
// Unknown or extensionless files default to "application/octet-stream".
// #############################################################################################
pub(crate) fn http_mime(path: &Path) -> &'static str {
    const MAX_EXTENSION_LENGTH: usize = 11;

    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return "application/octet-stream";
    };
    if extension.len() > MAX_EXTENSION_LENGTH {
        return "application/octet-stream";
    }

    let mut normalized = [0u8; MAX_EXTENSION_LENGTH];
    for (output, input) in normalized.iter_mut().zip(extension.bytes()) {
        *output = input.to_ascii_lowercase();
    }
    let Some(extension) = std::str::from_utf8(&normalized[..extension.len()]).ok() else {
        return "application/octet-stream";
    };

    text_mime(extension)
        .or_else(|| image_mime(extension))
        .or_else(|| audio_mime(extension))
        .or_else(|| video_mime(extension))
        .or_else(|| other_mime(extension))
        .unwrap_or("application/octet-stream")
}

// Return the text and structured-data MIME type for a normalized extension.
fn text_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "htm" | "html" => Some("text/html; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "csv" => Some("text/csv; charset=utf-8"),
        "md" | "markdown" => Some("text/markdown; charset=utf-8"),
        "txt" | "log" => Some("text/plain; charset=utf-8"),
        "js" | "mjs" | "cjs" => Some("application/javascript; charset=utf-8"),
        "json" | "map" | "ndjson" | "jsonl" => Some("application/json; charset=utf-8"),
        "webmanifest" => Some("application/manifest+json; charset=utf-8"),
        "atom" => Some("application/atom+xml; charset=utf-8"),
        "rss" => Some("application/rss+xml; charset=utf-8"),
        "xml" => Some("application/xml; charset=utf-8"),
        "yaml" | "yml" => Some("application/yaml; charset=utf-8"),
        "toml" => Some("application/toml; charset=utf-8"),
        "graphql" | "gql" => Some("application/graphql; charset=utf-8"),
        "sql" => Some("application/sql; charset=utf-8"),
        _ => None,
    }
}

// Return the image MIME type for a normalized extension.
fn image_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "avif" => Some("image/avif"),
        "bmp" => Some("image/bmp"),
        "gif" => Some("image/gif"),
        "ico" => Some("image/x-icon"),
        "jpeg" | "jpg" | "jfif" | "jif" | "jpe" | "pjpeg" | "pjp" => Some("image/jpeg"),
        "jxl" => Some("image/jxl"),
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        "tif" | "tiff" => Some("image/tiff"),
        "webp" => Some("image/webp"),
        "apng" => Some("image/apng"),
        "heic" | "heif" => Some("image/heic"),
        "psd" => Some("image/vnd.adobe.photoshop"),
        "raw" => Some("image/x-raw"),
        "cr2" => Some("image/x-canon-cr2"),
        "nef" => Some("image/x-nikon-nef"),
        "arw" => Some("image/x-sony-arw"),
        "dng" => Some("image/x-adobe-dng"),
        "dds" => Some("image/vnd.ms-dds"),
        "exr" => Some("image/aces"),
        "ppm" => Some("image/x-portable-pixmap"),
        "pgm" => Some("image/x-portable-graymap"),
        "pbm" => Some("image/x-portable-bitmap"),
        "pnm" => Some("image/x-portable-anymap"),
        "hdr" => Some("image/vnd.radiance"),
        _ => None,
    }
}

// Return the audio MIME type for a normalized extension.
fn audio_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "aac" | "m4a" => Some("audio/aac"),
        "ac3" => Some("audio/ac3"),
        "aiff" | "aif" | "aifc" => Some("audio/aiff"),
        "alac" | "m4b" | "m4p" => Some("audio/mp4"),
        "amr" => Some("audio/amr"),
        "ape" => Some("audio/ape"),
        "au" => Some("audio/basic"),
        "caf" => Some("audio/x-caf"),
        "dts" => Some("audio/vnd.dts"),
        "dtshd" => Some("audio/vnd.dts.hd"),
        "eac3" => Some("audio/eac3"),
        "flac" => Some("audio/flac"),
        "gsm" => Some("audio/gsm"),
        "m1a" | "m2a" | "m3a" | "mp1" | "mp2" | "mp3" | "mpa" | "mpga" => Some("audio/mpeg"),
        "mka" => Some("audio/x-matroska"),
        "mid" | "midi" | "kar" => Some("audio/midi"),
        "mod" | "s3m" | "xm" | "it" | "669" => Some("audio/x-mod"),
        "mpc" => Some("audio/x-musepack"),
        "oga" | "ogg" | "spx" => Some("audio/ogg"),
        "opus" => Some("audio/opus"),
        "ra" | "ram" => Some("audio/x-realaudio"),
        "tak" => Some("audio/x-tak"),
        "tta" => Some("audio/x-tta"),
        "voc" => Some("audio/x-voc"),
        "wav" | "wave" => Some("audio/wav"),
        "weba" => Some("audio/webm"),
        "wma" => Some("audio/x-ms-wma"),
        "wv" => Some("audio/wavpack"),
        _ => None,
    }
}

// Return the video MIME type for a normalized extension.
fn video_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "3gp" | "3gpp" => Some("video/3gpp"),
        "3g2" | "3gpp2" => Some("video/3gpp2"),
        "asf" => Some("video/x-ms-asf"),
        "avi" | "divx" => Some("video/x-msvideo"),
        "flv" => Some("video/x-flv"),
        "m2ts" | "mts" | "ts" => Some("video/mp2t"),
        "m4v" | "mp4" => Some("video/mp4"),
        "mkv" => Some("video/x-matroska"),
        "mov" => Some("video/quicktime"),
        "mpeg" | "mpg" | "mpe" => Some("video/mpeg"),
        "ogv" => Some("video/ogg"),
        "vob" => Some("video/dvd"),
        "webm" => Some("video/webm"),
        "wmv" => Some("video/x-ms-wmv"),
        _ => None,
    }
}

// Return MIME types for fonts, documents, archives, and WebAssembly.
fn other_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "eot" => Some("application/vnd.ms-fontobject"),
        "otf" => Some("font/otf"),
        "ttf" => Some("font/ttf"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        "pdf" => Some("application/pdf"),
        "7z" => Some("application/x-7z-compressed"),
        "gz" => Some("application/gzip"),
        "rar" => Some("application/vnd.rar"),
        "tar" => Some("application/x-tar"),
        "zip" => Some("application/zip"),
        "wasm" => Some("application/wasm"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::http_mime;
    use std::path::Path;

    #[test]
    fn recognizes_ascii_case_insensitive_extensions() {
        for (path, expected) in [
            ("index.HTML", "text/html; charset=utf-8"),
            ("app.Js", "application/javascript; charset=utf-8"),
            (
                "manifest.WEBMANIFEST",
                "application/manifest+json; charset=utf-8",
            ),
        ] {
            assert_eq!(http_mime(Path::new(path)), expected);
        }
    }

    #[test]
    fn unknown_extensions_use_the_binary_fallback() {
        assert_eq!(
            http_mime(Path::new("file.unknown")),
            "application/octet-stream"
        );
        assert_eq!(
            http_mime(Path::new("file.extension-that-is-too-long")),
            "application/octet-stream"
        );
    }
}
