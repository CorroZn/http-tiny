// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::Path; // Filesystem path type for working with file and directory paths.

// #############################################################################################
// MIME type function
// Will add the right MIME type to the HTTP response headers based on the file extension
// #############################################################################################
pub fn http_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        // HTML
        Some("html") | Some("htm") => "text/html",

        // Plain text
        Some("txt") => "text/plain",
        Some("css") => "text/css",
        Some("csv") => "text/csv",

        // JavaScript / JSON
        Some("js") => "application/javascript",
        Some("mjs") => "application/javascript",
        Some("json") => "application/json",
        Some("map") => "application/json",

        // XML
        Some("xml") => "application/xml",

        // Images
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        Some("tif") | Some("tiff") => "image/tiff",
        Some("jxl") => "image/jxl",

        // Audio
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("flac") => "audio/flac",
        Some("aac") => "audio/aac",
        Some("m4a") => "audio/mp4",

        // Video
        Some("mp4") => "video/mp4",
        Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("ogv") => "video/ogg",
        Some("mov") => "video/quicktime",
        Some("avi") => "video/x-msvideo",

        // Fonts
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("eot") => "application/vnd.ms-fontobject",

        // Documents and archives
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("tar") => "application/x-tar",
        Some("7z") => "application/x-7z-compressed",
        Some("rar") => "application/vnd.rar",

        // Web manifests
        Some("webmanifest") => "application/manifest+json",

        // Unknown file type.
        _ => "application/octet-stream",
    }
}
