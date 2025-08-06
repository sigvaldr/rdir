mod symbols;

use std::collections::HashMap;
use std::env;
use std::fs::{self, DirEntry, FileType, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
struct ColorScheme {
    reset: &'static str,
    dir: &'static str,
    symlink: &'static str,
    executable: &'static str,
    file: &'static str,
    pipe: &'static str,
    socket: &'static str,
    block_device: &'static str,
    char_device: &'static str,
    broken_symlink: &'static str,
    git_new: &'static str,
    git_modified: &'static str,
    git_deleted: &'static str,
    git_renamed: &'static str,
    git_untracked: &'static str,
    git_ignored: &'static str,
}

impl ColorScheme {
    const fn dark() -> Self {
        Self {
            reset: "\x1b[0m",
            dir: "\x1b[34m",
            symlink: "\x1b[36m",
            executable: "\x1b[32m",
            file: "\x1b[37m",
            pipe: "\x1b[33m",
            socket: "\x1b[35m",
            block_device: "\x1b[34m",
            char_device: "\x1b[33m",
            broken_symlink: "\x1b[31m",
            git_new: "\x1b[32m",
            git_modified: "\x1b[34m",
            git_deleted: "\x1b[31m",
            git_renamed: "\x1b[33m",
            git_untracked: "\x1b[90m",
            git_ignored: "\x1b[90m",
        }
    }

    const fn light() -> Self {
        Self {
            reset: "\x1b[0m",
            dir: "\x1b[94m",
            symlink: "\x1b[96m",
            executable: "\x1b[92m",
            file: "\x1b[30m",
            pipe: "\x1b[93m",
            socket: "\x1b[95m",
            block_device: "\x1b[94m",
            char_device: "\x1b[93m",
            broken_symlink: "\x1b[91m",
            git_new: "\x1b[92m",
            git_modified: "\x1b[94m",
            git_deleted: "\x1b[91m",
            git_renamed: "\x1b[93m",
            git_untracked: "\x1b[90m",
            git_ignored: "\x1b[90m",
        }
    }
}

#[derive(Clone)]
struct Options {
    one_per_line: bool,
    all: bool,
    almost_all: bool,
    dirs_only: bool,
    files_only: bool,
    long: bool,
    report: bool,
    tree_depth: Option<usize>,
    git_status: bool,
    sort_dirs_first: bool,
    sort_files_first: bool,
    sort_time: bool,
    human_readable: bool,
    color_scheme: ColorScheme,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            one_per_line: false,
            all: false,
            almost_all: false,
            dirs_only: false,
            files_only: false,
            long: false,
            report: false,
            tree_depth: None,
            git_status: false,
            sort_dirs_first: false,
            sort_files_first: false,
            sort_time: false,
            human_readable: true,
            color_scheme: ColorScheme::dark(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitState {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Untracked,
    Ignored,
    None,
}

struct EntryInfo {
    entry: DirEntry,
    metadata: Metadata,
    icon: &'static str,
    git_state: GitState,
}

#[derive(Default)]
struct Counts {
    dirs: usize,
    files: usize,
    symlinks: usize,
    pipes: usize,
    sockets: usize,
    block_devices: usize,
    char_devices: usize,
    broken_symlinks: usize,
}

fn main() {
    let mut opts = Options::default();
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut args = env::args().skip(1);
    
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-1" => opts.one_per_line = true,
            "-a" | "--all" => opts.all = true,
            "-A" | "--almost-all" => {
                opts.all = true;
                opts.almost_all = true;
            }
            "-d" | "--dirs" => opts.dirs_only = true,
            "-f" | "--files" => opts.files_only = true,
            "-l" | "--long" => opts.long = true,
            "--report" => opts.report = true,
            s if s.starts_with("--tree") => {
                if s == "--tree" {
                    opts.tree_depth = Some(3);
                } else if let Some(eq_idx) = s.find('=') {
                    let val = &s[eq_idx + 1..];
                    if val.is_empty() {
                        opts.tree_depth = Some(3);
                    } else if let Ok(num) = val.parse::<isize>() {
                        if num <= 0 {
                            opts.tree_depth = None;
                        } else {
                            opts.tree_depth = Some(num as usize);
                        }
                    } else {
                        eprintln!("Invalid depth for --tree: {}", val);
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Invalid syntax for --tree: {}", s);
                    std::process::exit(1);
                }
            }
            "--gs" | "--git-status" => opts.git_status = true,
            "--sd" | "--sort-dirs" | "--group-directories-first" => opts.sort_dirs_first = true,
            "--sf" | "--sort-files" => opts.sort_files_first = true,
            "-t" => opts.sort_time = true,
            "--light" => opts.color_scheme = ColorScheme::light(),
            "--dark" => opts.color_scheme = ColorScheme::dark(),
            "--non-human-readable" => opts.human_readable = false,
            "--help" | "-h" => {
                print_help();
                return;
            }
            s if s.starts_with('-') => {
                eprintln!("Unknown flag: {}", s);
                std::process::exit(1);
            }
            _ => {
                paths.push(PathBuf::from(arg));
            }
        }
    }
    
    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    
    let multiple = paths.len() > 1;
    for (idx, path) in paths.iter().enumerate() {
        if multiple {
            println!("{}:", path.display());
        }
        
        let mut counts = Counts::default();
        
        if let Some(depth) = opts.tree_depth {
            let git_map = if opts.git_status {
                git_statuses(path)
            } else {
                HashMap::new()
            };
            print_tree(path, path, "".to_string(), depth, &opts, &git_map, &mut counts);
        } else if opts.tree_depth.is_some() {
            let git_map = if opts.git_status {
                git_statuses(path)
            } else {
                HashMap::new()
            };
            print_tree(path, path, "".to_string(), usize::MAX, &opts, &git_map, &mut counts);
        } else {
            list_dir(path, &opts, &mut counts);
        }
        
        if opts.report {
            print_report(&counts);
        }
        
        if multiple && idx + 1 < paths.len() {
            println!();
        }
    }
}

fn print_help() {
    let help = "rDir: a Rust implementation of directory listing\n\n\
Usage: rDir [OPTIONS] [PATH]...\n\
If no PATH is given, the current directory is listed.  Multiple paths\n\
may be given and will be listed in sequence.\n\n\
Options:\n\
  -1                     List one entry per line (disables column view)\n\
  -a, --all              Do not ignore entries starting with '.'\n\
  -A, --almost-all       Like -a but excludes '.' and '..' (read_dir already excludes them)\n\
  -d, --dirs             Show only directories\n\
  -f, --files            Show only files\n\
  -l, --long             Use a long listing format (perms, links, uid, gid, size, date)\n\
  --report              Show a summary of the number of files and folders displayed\n\
  --tree[=DEPTH]         Recurse into directories and show a tree view.\n\
                         Omitting DEPTH uses a default of 3.  A DEPTH of 0\n\
                         or a negative number prints the entire tree.\n\
  --gs, --git-status     Show git status for each entry (if inside a git repository)\n\
  --sd, --sort-dirs      Group directories before files (mutually exclusive with --sf)\n\
  --sf, --sort-files     Group files before directories (mutually exclusive with --sd)\n\
  -t                     Sort entries by modification time, newest first\n\
  --light                Use a light colour scheme (for light terminal backgrounds)\n\
  --dark                 Use the default dark colour scheme (default)\n\
  --non-human-readable   Print file sizes in bytes rather than a human readable format\n\
  -h, --help             Print this help message\n";
    print!("{}", help);
    io::stdout().flush().unwrap();
}

fn git_statuses(path: &Path) -> HashMap<PathBuf, GitState> {
    let mut map: HashMap<PathBuf, GitState> = HashMap::new();
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(path)
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                for line in stdout.lines() {
                    if line.len() < 3 {
                        continue;
                    }
                    let x = line.as_bytes()[0] as char;
                    let y = line.as_bytes()[1] as char;
                    let remainder = &line[3..];
                    let rel_path = if let Some(idx) = remainder.find(" -> ") {
                        PathBuf::from(&remainder[idx + 4..])
                    } else {
                        PathBuf::from(remainder)
                    };
                    let state = parse_git_state(x, y);
                    map.insert(rel_path, state);
                }
            }
        }
    }
    map
}

fn parse_git_state(x: char, y: char) -> GitState {
    let c = if x != ' ' { x } else { y };
    match c {
        'A' | 'C' => GitState::Added,
        'M' => GitState::Modified,
        'D' => GitState::Deleted,
        'R' => GitState::Renamed,
        'T' => GitState::TypeChanged,
        '?' => GitState::Untracked,
        '!' => GitState::Ignored,
        _ => GitState::None,
    }
}

fn perm_string(file_type: &FileType, metadata: &Metadata) -> String {
    let mut s = String::new();
    
    let type_char = if file_type.is_dir() {
        'd'
    } else if file_type.is_symlink() {
        'l'
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if file_type.is_fifo() {
                'p'
            } else if file_type.is_socket() {
                's'
            } else if file_type.is_block_device() {
                'b'
            } else if file_type.is_char_device() {
                'c'
            } else {
                '-'
            }
        }
        #[cfg(not(unix))]
        {
            '-'
        }
    };
    s.push(type_char);
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
        s.push(if mode & 0o200 != 0 { 'w' } else { '-' });
        s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
        s.push(if mode & 0o40 != 0 { 'r' } else { '-' });
        s.push(if mode & 0o20 != 0 { 'w' } else { '-' });
        s.push(if mode & 0o10 != 0 { 'x' } else { '-' });
        s.push(if mode & 0o4 != 0 { 'r' } else { '-' });
        s.push(if mode & 0o2 != 0 { 'w' } else { '-' });
        s.push(if mode & 0o1 != 0 { 'x' } else { '-' });
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        for _ in 0..9 {
            s.push('-');
        }
    }
    s
}

fn format_time(st: SystemTime) -> String {
    let duration = match st.duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(e) => e.duration(),
    };
    let secs = duration.as_secs();
    let days = secs / 86_400;
    let mut rem_secs = secs % 86_400;
    let hour = (rem_secs / 3_600) as u32;
    rem_secs %= 3_600;
    let minute = (rem_secs / 60) as u32;
    
    let mut year: i32 = 1970;
    let mut day_count = days as i64;
    
    loop {
        let leap = is_leap_year(year);
        let year_days = if leap { 366 } else { 365 };
        if day_count >= year_days {
            day_count -= year_days;
            year += 1;
        } else {
            break;
        }
    }
    
    let month_lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let month_lengths_leap = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let months = if is_leap_year(year) { &month_lengths_leap } else { &month_lengths };
    let mut month: usize = 0;
    while day_count >= months[month] as i64 {
        day_count -= months[month] as i64;
        month += 1;
    }
    let day = day_count + 1;
    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month + 1, day, hour, minute)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn format_size(size: u64, human_readable: bool) -> String {
    if !human_readable {
        return size.to_string();
    }
    let units = ["B", "K", "M", "G", "T", "P", "E", "Z", "Y"];
    let mut s = size as f64;
    let mut idx = 0;
    while s >= 1024.0 && idx < units.len() - 1 {
        s /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{}{}", size, units[idx])
    } else {
        format!("{:.1}{}", s, units[idx])
    }
}

fn visible_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut len = 0;
    let mut in_escape = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_escape {
            if b == b'm' {
                in_escape = false;
            }
        } else {
            if b == 0x1b {
                if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                    in_escape = true;
                    i += 1;
                } else {
                    len += 1;
                }
            } else {
                len += 1;
            }
        }
        i += 1;
    }
    len
}

fn list_dir(path: &Path, opts: &Options, counts: &mut Counts) {
    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => {
            eprintln!("rDir: cannot access {}: {}", path.display(), e);
            return;
        }
    };
    
    let git_map = if opts.git_status {
        git_statuses(path)
    } else {
        HashMap::new()
    };
    
    let mut entries: Vec<EntryInfo> = Vec::new();
    for res in read_dir {
        match res {
            Ok(entry) => {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                
                if !opts.all {
                    if file_name_str.starts_with('.') {
                        continue;
                    }
                }
                
                let metadata = match fs::symlink_metadata(entry.path()) {
                    Ok(md) => md,
                    Err(_) => continue,
                };
                
                let file_type = metadata.file_type();
                
                if opts.dirs_only && !file_type.is_dir() {
                    continue;
                }
                if opts.files_only && file_type.is_dir() {
                    continue;
                }
                
                let rel_path = match entry.path().strip_prefix(path) {
                    Ok(p) => p.to_owned(),
                    Err(_) => entry.path(),
                };
                let git_state = git_map.get(&rel_path).cloned().unwrap_or(GitState::None);
                let icon = symbols::get_file_icon(&file_type, &entry.path());
                
                if file_type.is_dir() {
                    counts.dirs += 1;
                } else if file_type.is_symlink() {
                    if fs::read_link(entry.path()).map_or(true, |tgt| tgt.exists()) {
                        counts.symlinks += 1;
                    } else {
                        counts.broken_symlinks += 1;
                    }
                } else {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::FileTypeExt;
                        if file_type.is_fifo() {
                            counts.pipes += 1;
                        } else if file_type.is_socket() {
                            counts.sockets += 1;
                        } else if file_type.is_block_device() {
                            counts.block_devices += 1;
                        } else if file_type.is_char_device() {
                            counts.char_devices += 1;
                        } else {
                            counts.files += 1;
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        counts.files += 1;
                    }
                }
                
                entries.push(EntryInfo {
                    entry,
                    metadata,
                    icon,
                    git_state,
                });
            }
            Err(e) => {
                eprintln!("rDir: error reading directory: {}", e);
            }
        }
    }
    
    entries.sort_by(|a, b| {
        let a_dir = a.metadata.file_type().is_dir();
        let b_dir = b.metadata.file_type().is_dir();
        
        if opts.sort_dirs_first && a_dir != b_dir {
            if a_dir { return std::cmp::Ordering::Less; }
            else { return std::cmp::Ordering::Greater; }
        }
        if opts.sort_files_first && a_dir != b_dir {
            if a_dir { return std::cmp::Ordering::Greater; }
            else { return std::cmp::Ordering::Less; }
        }
        if opts.sort_time {
            let a_time = a.metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let b_time = b.metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            match b_time.cmp(&a_time) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }
        
        let a_name = a.entry.file_name().to_string_lossy().to_lowercase();
        let b_name = b.entry.file_name().to_string_lossy().to_lowercase();
        a_name.cmp(&b_name)
    });
    
    if opts.long {
        let mut link_w = 0;
        let mut uid_w = 0;
        let mut gid_w = 0;
        let mut size_w = 0;
        
        for info in &entries {
            let links: u64 = {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    info.metadata.nlink() as u64
                }
                #[cfg(not(unix))]
                {
                    1
                }
            };
            link_w = link_w.max(format!("{}", links).len());
            
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let uid = info.metadata.uid();
                let gid = info.metadata.gid();
                uid_w = uid_w.max(format!("{}", uid).len());
                gid_w = gid_w.max(format!("{}", gid).len());
            }
            #[cfg(not(unix))]
            {
                uid_w = uid_w.max(1);
                gid_w = gid_w.max(1);
            }
            
            let size = info.metadata.len();
            let size_str = format_size(size, opts.human_readable);
            size_w = size_w.max(size_str.len());
        }
        
        for info in entries {
            print_long_entry(info, link_w, uid_w, gid_w, size_w, opts);
        }
    } else {
        let mut display_strings: Vec<String> = Vec::new();
        let mut max_len = 0;
        
        for info in &entries {
            let s = build_short_display(info, opts);
            max_len = max_len.max(visible_len(&s));
            display_strings.push(s);
        }
        
        let term_width: usize = match env::var("COLUMNS") {
            Ok(val) => val.parse().unwrap_or(80),
            Err(_) => 80,
        };
        
        let col_width = max_len + 2;
        let cols = if opts.one_per_line {
            1
        } else if col_width == 0 {
            1
        } else {
            let c = term_width / col_width;
            if c == 0 { 1 } else { c }
        };
        
        let rows = (display_strings.len() + cols - 1) / cols;
        for r in 0..rows {
            let mut line = String::new();
            for c in 0..cols {
                let idx = r + c * rows;
                if idx < display_strings.len() {
                    let s = &display_strings[idx];
                    let vis_len = visible_len(s);
                    line.push_str(s);
                    if c + 1 < cols {
                        let pad = col_width - vis_len;
                        for _ in 0..pad {
                            line.push(' ');
                        }
                    }
                }
            }
            println!("{}", line);
        }
    }
}

fn build_short_display(info: &EntryInfo, opts: &Options) -> String {
    let scheme = opts.color_scheme;
    let file_type = info.metadata.file_type();
    let mut parts = String::new();
    
    match info.git_state {
        GitState::Added => {
            parts.push_str(scheme.git_new);
            parts.push('A');
            parts.push_str(scheme.reset);
        }
        GitState::Modified => {
            parts.push_str(scheme.git_modified);
            parts.push('M');
            parts.push_str(scheme.reset);
        }
        GitState::Deleted => {
            parts.push_str(scheme.git_deleted);
            parts.push('D');
            parts.push_str(scheme.reset);
        }
        GitState::Renamed => {
            parts.push_str(scheme.git_renamed);
            parts.push('R');
            parts.push_str(scheme.reset);
        }
        GitState::TypeChanged => {
            parts.push_str(scheme.git_renamed);
            parts.push('T');
            parts.push_str(scheme.reset);
        }
        GitState::Untracked => {
            parts.push_str(scheme.git_untracked);
            parts.push('?');
            parts.push_str(scheme.reset);
        }
        GitState::Ignored => {
            parts.push_str(scheme.git_ignored);
            parts.push('I');
            parts.push_str(scheme.reset);
        }
        GitState::None => {
            parts.push(' ');
        }
    }
    parts.push(' ');
    
    let icon_color = if file_type.is_dir() {
        scheme.dir
    } else if file_type.is_symlink() {
        if fs::read_link(info.entry.path()).map_or(true, |tgt| tgt.exists()) {
            scheme.symlink
        } else {
            scheme.broken_symlink
        }
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if file_type.is_fifo() {
                scheme.pipe
            } else if file_type.is_socket() {
                scheme.socket
            } else if file_type.is_block_device() {
                scheme.block_device
            } else if file_type.is_char_device() {
                scheme.char_device
            } else if is_executable(&info.metadata) {
                scheme.executable
            } else {
                scheme.file
            }
        }
        #[cfg(not(unix))]
        {
            if is_executable(&info.metadata) {
                scheme.executable
            } else {
                scheme.file
            }
        }
    };
    
    parts.push_str(icon_color);
    parts.push_str(info.icon);
    parts.push_str(scheme.reset);
    parts.push(' ');
    
    let name_color = if file_type.is_dir() {
        scheme.dir
    } else if file_type.is_symlink() {
        if fs::read_link(info.entry.path()).map_or(true, |tgt| tgt.exists()) {
            scheme.symlink
        } else {
            scheme.broken_symlink
        }
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if file_type.is_fifo() {
                scheme.pipe
            } else if file_type.is_socket() {
                scheme.socket
            } else if file_type.is_block_device() {
                scheme.block_device
            } else if file_type.is_char_device() {
                scheme.char_device
            } else if is_executable(&info.metadata) {
                scheme.executable
            } else {
                scheme.file
            }
        }
        #[cfg(not(unix))]
        {
            if is_executable(&info.metadata) {
                scheme.executable
            } else {
                scheme.file
            }
        }
    };
    
    let file_name = info.entry.file_name();
    let file_name_str = file_name.to_string_lossy();
    parts.push_str(name_color);
    parts.push_str(&file_name_str);
    
    if file_type.is_symlink() {
        match fs::read_link(info.entry.path()) {
            Ok(target) => {
                parts.push_str(scheme.reset);
                parts.push_str(" -> ");
                let target_str = target.to_string_lossy();
                parts.push_str(name_color);
                parts.push_str(&target_str);
            }
            Err(_) => {}
        }
    }
    parts.push_str(scheme.reset);
    parts
}

fn print_long_entry(info: EntryInfo, link_w: usize, uid_w: usize, gid_w: usize, size_w: usize, opts: &Options) {
    let scheme = opts.color_scheme;
    let file_type = info.metadata.file_type();
    let perm = perm_string(&file_type, &info.metadata);
    
    let links: u64 = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            info.metadata.nlink() as u64
        }
        #[cfg(not(unix))]
        {
            1
        }
    };
    
    #[cfg(unix)]
    let (uid_num, gid_num) = {
        use std::os::unix::fs::MetadataExt;
        (info.metadata.uid(), info.metadata.gid())
    };
    #[cfg(not(unix))]
    let (uid_num, gid_num) = (0_u32, 0_u32);
    
    let uid_str = format!("{}", uid_num);
    let gid_str = format!("{}", gid_num);
    
    let size = info.metadata.len();
    let size_str = format_size(size, opts.human_readable);
    
    let mtime = info.metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let time_str = format_time(mtime);
    
    let git_ch = match info.git_state {
        GitState::Added => {
            format!("{}A{}", scheme.git_new, scheme.reset)
        }
        GitState::Modified => {
            format!("{}M{}", scheme.git_modified, scheme.reset)
        }
        GitState::Deleted => {
            format!("{}D{}", scheme.git_deleted, scheme.reset)
        }
        GitState::Renamed => {
            format!("{}R{}", scheme.git_renamed, scheme.reset)
        }
        GitState::TypeChanged => {
            format!("{}T{}", scheme.git_renamed, scheme.reset)
        }
        GitState::Untracked => {
            format!("{}?{}", scheme.git_untracked, scheme.reset)
        }
        GitState::Ignored => {
            format!("{}I{}", scheme.git_ignored, scheme.reset)
        }
        GitState::None => " ".to_string(),
    };
    
    let short = build_short_display(&info, opts);
    
    print!("{} ", perm);
    print!("{:>width$} ", links, width = link_w);
    print!(" {:>uid_w$} ", uid_str, uid_w = uid_w);
    print!(" {:>gid_w$} ", gid_str, gid_w = gid_w);
    print!(" {:>size_w$} ", size_str, size_w = size_w);
    print!(" {} {} ", time_str, git_ch);
    println!("{}", short);
}

fn is_executable(metadata: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        mode & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn print_tree(current: &Path, root: &Path, prefix: String, depth: usize, opts: &Options, git_map: &HashMap<PathBuf, GitState>, counts: &mut Counts) {
    let read_dir = match fs::read_dir(current) {
        Ok(rd) => rd,
        Err(e) => {
            eprintln!("rDir: cannot access {}: {}", current.display(), e);
            return;
        }
    };
    
    let mut entries: Vec<EntryInfo> = Vec::new();
    for res in read_dir {
        match res {
            Ok(entry) => {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                
                if !opts.all {
                    if file_name_str.starts_with('.') {
                        continue;
                    }
                }
                
                let metadata = match fs::symlink_metadata(entry.path()) {
                    Ok(md) => md,
                    Err(_) => continue,
                };
                
                let file_type = metadata.file_type();
                
                if opts.dirs_only && !file_type.is_dir() {
                    continue;
                }
                if opts.files_only && file_type.is_dir() {
                    continue;
                }
                
                let rel_path = match entry.path().strip_prefix(root) {
                    Ok(p) => p.to_owned(),
                    Err(_) => entry.path(),
                };
                let git_state = git_map.get(&rel_path).cloned().unwrap_or(GitState::None);
                let icon = symbols::get_file_icon(&file_type, &entry.path());
                
                if file_type.is_dir() {
                    counts.dirs += 1;
                } else if file_type.is_symlink() {
                    if fs::read_link(entry.path()).map_or(true, |tgt| tgt.exists()) {
                        counts.symlinks += 1;
                    } else {
                        counts.broken_symlinks += 1;
                    }
                } else {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::FileTypeExt;
                        if file_type.is_fifo() {
                            counts.pipes += 1;
                        } else if file_type.is_socket() {
                            counts.sockets += 1;
                        } else if file_type.is_block_device() {
                            counts.block_devices += 1;
                        } else if file_type.is_char_device() {
                            counts.char_devices += 1;
                        } else {
                            counts.files += 1;
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        counts.files += 1;
                    }
                }
                
                entries.push(EntryInfo {
                    entry,
                    metadata,
                    icon,
                    git_state,
                });
            }
            Err(e) => {
                eprintln!("rDir: error reading directory: {}", e);
            }
        }
    }
    
    entries.sort_by(|a, b| {
        let a_dir = a.metadata.file_type().is_dir();
        let b_dir = b.metadata.file_type().is_dir();
        if a_dir != b_dir {
            if a_dir { return std::cmp::Ordering::Less; }
            else { return std::cmp::Ordering::Greater; }
        }
        
        if opts.sort_time {
            let a_time = a.metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let b_time = b.metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            match b_time.cmp(&a_time) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }
        
        let a_name = a.entry.file_name().to_string_lossy().to_lowercase();
        let b_name = b.entry.file_name().to_string_lossy().to_lowercase();
        a_name.cmp(&b_name)
    });
    
    let len = entries.len();
    for (i, info) in entries.into_iter().enumerate() {
        let is_last = i == len - 1;
        
        let mut line = prefix.clone();
        if is_last {
            line.push_str("└── ");
        } else {
            line.push_str("├── ");
        }
        
        let disp = build_short_display(&info, opts);
        println!("{}{}", line, disp);
        
        if info.metadata.file_type().is_dir() {
            let new_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };
            if depth > 1 {
                print_tree(&info.entry.path(), root, new_prefix, depth - 1, opts, git_map, counts);
            } else if depth == usize::MAX {
                print_tree(&info.entry.path(), root, new_prefix, usize::MAX, opts, git_map, counts);
            }
        }
    }
}

fn print_report(counts: &Counts) {
    let mut parts: Vec<String> = Vec::new();
    if counts.dirs > 0 {
        parts.push(format!("{} director{}", counts.dirs, if counts.dirs == 1 { "y" } else { "ies" }));
    }
    if counts.files > 0 {
        parts.push(format!("{} file{}", counts.files, if counts.files == 1 { "" } else { "s" }));
    }
    if counts.symlinks > 0 {
        parts.push(format!("{} symlink{}", counts.symlinks, if counts.symlinks == 1 { "" } else { "s" }));
    }
    if counts.broken_symlinks > 0 {
        parts.push(format!("{} broken symlink{}", counts.broken_symlinks, if counts.broken_symlinks == 1 { "" } else { "s" }));
    }
    if counts.pipes > 0 {
        parts.push(format!("{} pipe{}", counts.pipes, if counts.pipes == 1 { "" } else { "s" }));
    }
    if counts.sockets > 0 {
        parts.push(format!("{} socket{}", counts.sockets, if counts.sockets == 1 { "" } else { "s" }));
    }
    if counts.block_devices > 0 {
        parts.push(format!("{} block device{}", counts.block_devices, if counts.block_devices == 1 { "" } else { "s" }));
    }
    if counts.char_devices > 0 {
        parts.push(format!("{} char device{}", counts.char_devices, if counts.char_devices == 1 { "" } else { "s" }));
    }
    if !parts.is_empty() {
        println!("\n{}", parts.join(", "));
    }
}