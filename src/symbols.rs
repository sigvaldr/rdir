pub const DIRECTORY: &str = "📁";
pub const SYMLINK: &str = "🔗";
pub const PIPE: &str = "│";
pub const SOCKET: &str = "🔌";
pub const BLOCK_DEVICE: &str = "⬛";
pub const CHAR_DEVICE: &str = "📟";
pub const GENERIC_FILE: &str = "📄";

pub const RUST: &str = "🦀";
pub const RUBY: &str = "💎";
pub const PYTHON: &str = "🐍";
pub const JAVASCRIPT: &str = "📜";
pub const GO: &str = "🐹";
pub const SHELL: &str = "🐚";
pub const C_CPP: &str = "📄";
pub const JAVA: &str = "☕";
pub const MARKDOWN: &str = "📘";
pub const TEXT: &str = "📄";
pub const JSON: &str = "🗂";
pub const CONFIG: &str = "🧾";
pub const HTML: &str = "🌐";
pub const CSS: &str = "🎨";
pub const ARCHIVE: &str = "📦";
pub const IMAGE: &str = "🖼";
pub const AUDIO: &str = "🎵";
pub const VIDEO: &str = "🎬";
pub const PDF: &str = "📄";
pub const DOCUMENT: &str = "📄";
pub const PRESENTATION: &str = "📊";
pub const SPREADSHEET: &str = "📊";
pub const DATABASE: &str = "🗄";
pub const LOG: &str = "📜";
pub const LOCK: &str = "🔒";

pub fn get_file_icon(file_type: &std::fs::FileType, path: &std::path::Path) -> &'static str {
    if file_type.is_dir() {
        return DIRECTORY;
    }
    if file_type.is_symlink() {
        return SYMLINK;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if file_type.is_fifo() {
            return PIPE;
        }
        if file_type.is_socket() {
            return SOCKET;
        }
        if file_type.is_block_device() {
            return BLOCK_DEVICE;
        }
        if file_type.is_char_device() {
            return CHAR_DEVICE;
        }
    }

    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_ascii_lowercase();
        match ext.as_str() {
            "rs" => RUST,
            "rb" => RUBY,
            "py" => PYTHON,
            "js" | "ts" => JAVASCRIPT,
            "go" => GO,
            "sh" | "zsh" | "bash" => SHELL,
            "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" => C_CPP,
            "java" => JAVA,
            "md" | "markdown" => MARKDOWN,
            "txt" | "text" => TEXT,
            "json" => JSON,
            "toml" | "yaml" | "yml" => CONFIG,
            "html" | "htm" => HTML,
            "css" => CSS,
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => ARCHIVE,
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" => IMAGE,
            "mp3" | "flac" | "ogg" | "wav" | "aac" => AUDIO,
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" => VIDEO,
            "pdf" => PDF,
            "doc" | "docx" | "odt" | "rtf" => DOCUMENT,
            "ppt" | "pptx" | "odp" => PRESENTATION,
            "xls" | "xlsx" | "ods" | "csv" => SPREADSHEET,
            "sql" | "db" | "sqlite" => DATABASE,
            "log" => LOG,
            "lock" => LOCK,
            _ => GENERIC_FILE,
        }
    } else {
        GENERIC_FILE
    }
} 