// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::Path; // Filesystem path type for representing file and directory paths.

// #############################################################################################
// MIME type function
// Returns the appropriate MIME type for a file based on its extension.
// Unknown or extensionless files default to "application/octet-stream".
// #############################################################################################
pub fn http_mime(path: &Path) -> &'static str {

    // Determine the MIME type based on the file extension.
    let extension = path
    .extension()
    .and_then(|ext| ext.to_str())
    // Normalize uppercase file extensions to lowercase for matching.
    .map(|ext| ext.to_ascii_lowercase());

    // Supported file extensions and their corresponding MIME types.
    match extension.as_deref() {
        // HTML
        Some("htm" | "html") => "text/html; charset=utf-8",

        // Plain text
        Some("css") => "text/css; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("md" | "markdown") => "text/markdown; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("log") => "text/plain; charset=utf-8",

        // JavaScript
        Some("js" | "mjs" | "cjs") => "application/javascript; charset=utf-8",

        // JSON
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("webmanifest") => "application/manifest+json; charset=utf-8",
        Some("ndjson" | "jsonl") => "application/json; charset=utf-8",

        // XML
        Some("atom") => "application/atom+xml; charset=utf-8",
        Some("rss") => "application/rss+xml; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",

        // Structured data and configuration formats
        Some("yaml" | "yml") => "application/yaml; charset=utf-8",
        Some("toml") => "application/toml; charset=utf-8",
        Some("graphql" | "gql") => "application/graphql; charset=utf-8",
        Some("sql") => "application/sql; charset=utf-8",

        // Images
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("jxl") => "image/jxl",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("tif" | "tiff") => "image/tiff",
        Some("webp") => "image/webp",
        Some("apng") => "image/apng",
        Some("jfif" | "jif" | "jpe") => "image/jpeg",
        Some("pjpeg" | "pjp") => "image/jpeg",
        Some("heic" | "heif") => "image/heic",
        Some("psd") => "image/vnd.adobe.photoshop",
        Some("raw") => "image/x-raw",
        Some("cr2") => "image/x-canon-cr2",
        Some("nef") => "image/x-nikon-nef",
        Some("arw") => "image/x-sony-arw",
        Some("dng") => "image/x-adobe-dng",
        Some("dds") => "image/vnd.ms-dds",
        Some("exr") => "image/aces",
        Some("ppm") => "image/x-portable-pixmap",
        Some("pgm") => "image/x-portable-graymap",
        Some("pbm") => "image/x-portable-bitmap",
        Some("pnm") => "image/x-portable-anymap",
        Some("hdr") => "image/vnd.radiance",

        // Audio
        Some("aac" | "m4a") => "audio/aac",
        Some("ac3") => "audio/ac3",
        Some("aiff" | "aif" | "aifc") => "audio/aiff",
        Some("alac") => "audio/mp4",
        Some("amr") => "audio/amr",
        Some("ape") => "audio/ape",
        Some("au") => "audio/basic",
        Some("caf") => "audio/x-caf",
        Some("dts") => "audio/vnd.dts",
        Some("dtshd") => "audio/vnd.dts.hd",
        Some("eac3") => "audio/eac3",
        Some("flac") => "audio/flac",
        Some("gsm") => "audio/gsm",
        Some("m1a" | "m2a" | "m3a" | "mp1" | "mp2" | "mp3" | "mpa" | "mpga") => "audio/mpeg",
        Some("m4b" | "m4p") => "audio/mp4",
        Some("mka") => "audio/x-matroska",
        Some("mid" | "midi" | "kar") => "audio/midi",
        Some("mod" | "s3m" | "xm" | "it" | "669") => "audio/x-mod",
        Some("mpc") => "audio/x-musepack",
        Some("oga" | "ogg" | "spx") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("ra" | "ram") => "audio/x-realaudio",
        Some("tak") => "audio/x-tak",
        Some("tta") => "audio/x-tta",
        Some("voc") => "audio/x-voc",
        Some("wav" | "wave") => "audio/wav",
        Some("weba") => "audio/webm",
        Some("wma") => "audio/x-ms-wma",
        Some("wv") => "audio/wavpack",

        // Video
        Some("3gp" | "3gpp") => "video/3gpp",
        Some("3g2" | "3gpp2") => "video/3gpp2",
        Some("asf") => "video/x-ms-asf",
        Some("avi") => "video/x-msvideo",
        Some("divx") => "video/x-msvideo",
        Some("flv") => "video/x-flv",
        Some("m2ts" | "mts") => "video/mp2t",
        Some("m4v" | "mp4") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("mov") => "video/quicktime",
        Some("mpeg" | "mpg" | "mpe") => "video/mpeg",
        Some("ogv") => "video/ogg",
        Some("ts") => "video/mp2t",
        Some("vob") => "video/dvd",
        Some("webm") => "video/webm",
        Some("wmv") => "video/x-ms-wmv",

        // Fonts
        Some("eot") => "application/vnd.ms-fontobject",
        Some("otf") => "font/otf",
        Some("ttf") => "font/ttf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",

        // Documents
        Some("pdf") => "application/pdf",

        // Compressed archives
        Some("7z") => "application/x-7z-compressed",
        Some("gz") => "application/gzip",
        Some("rar") => "application/vnd.rar",
        Some("tar") => "application/x-tar",
        Some("zip") => "application/zip",

        // WebAssembly
        Some("wasm") => "application/wasm",

        // Unknown file type
        _ => "application/octet-stream",
    }
}