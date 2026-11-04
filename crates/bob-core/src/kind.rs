use std::path::Path;

pub fn kind_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "tif" | "tiff" | "raw"
        | "svg" | "ico" | "avif" => "imagem",
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "m4v" | "mpg" | "mpeg" => "video",
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "opus" | "wma" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "cab" => "arquivo",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf" | "odt"
        | "ods" | "csv" | "md" => "documento",
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "cpp" | "h" | "cs"
        | "json" | "xml" | "html" | "css" | "toml" | "yml" | "yaml" => "codigo",
        "exe" | "msi" | "dll" | "bat" | "cmd" | "ps1" | "lnk" => "programa",
        _ => "outro",
    }
}

pub fn is_image_ext(path: &Path) -> bool {
    kind_of(path) == "imagem" && path.extension().is_some_and(|ext| ext != "svg" && ext != "ico")
}
