use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut lenient = false;
    let mut max_depth: Option<u32> = None;
    let mut path: Option<String> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--lenient" => lenient = true,
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "--max-depth" => {
                i += 1;
                let value = match args.get(i) {
                    Some(v) => v,
                    None => {
                        eprintln!("strictdu: --max-depth requires a value");
                        return ExitCode::from(2);
                    }
                };
                match value.parse() {
                    Ok(n) => max_depth = Some(n),
                    Err(_) => {
                        eprintln!("strictdu: invalid --max-depth value '{value}'");
                        return ExitCode::from(2);
                    }
                }
            }
            other if other.starts_with("--max-depth=") => {
                let value = &other["--max-depth=".len()..];
                match value.parse() {
                    Ok(n) => max_depth = Some(n),
                    Err(_) => {
                        eprintln!("strictdu: invalid --max-depth value '{value}'");
                        return ExitCode::from(2);
                    }
                }
            }
            other => {
                if path.is_some() {
                    eprintln!("strictdu: unexpected extra argument '{other}'");
                    return ExitCode::from(2);
                }
                path = Some(other.to_string());
            }
        }
        i += 1;
    }

    let root = PathBuf::from(path.unwrap_or_else(|| ".".to_string()));
    let options = strictdu::ScanOptions { lenient, max_depth };

    match strictdu::scan(&root, options) {
        Ok(report) => {
            for entry in &report.entries {
                print_entry(entry, 0);
            }
            println!("{:>10}  total ({})", human_size(report.total_bytes), report.root.display());

            if !report.warnings.is_empty() {
                eprintln!();
                eprintln!("{} item(s) skipped (--lenient):", report.warnings.len());
                for warning in &report.warnings {
                    eprintln!("  {}: {}", warning.path.display(), warning.message);
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("strictdu: {e}");
            eprintln!("strictdu: pass --lenient to skip unreadable entries instead of aborting");
            ExitCode::FAILURE
        }
    }
}

fn print_entry(entry: &strictdu::Entry, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{:>10}  {indent}{}", human_size(entry.bytes), entry.path.display());
    for child in &entry.children {
        print_entry(child, depth + 1);
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} {}", UNITS[0]);
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

fn print_help() {
    println!("strictdu [--lenient] [--max-depth N] [PATH]");
    println!();
    println!("Report the size of each direct child of PATH, largest first.");
    println!("Defaults PATH to the current directory.");
    println!();
    println!("By default, any unreadable file, denied permission, or non-regular");
    println!("entry (broken symlink, socket, device file) aborts the scan with an");
    println!("error instead of quietly leaving it out of the total.");
    println!();
    println!("  --lenient        skip unreadable entries and print a warning");
    println!("                   summary on stderr instead of aborting");
    println!("  --max-depth N    also break out entries N levels below PATH,");
    println!("                   indented under their parent (default 1: only");
    println!("                   PATH's direct children). The grand total is");
    println!("                   always computed from the full tree either way.");
    println!("  -h, --help       print this message");
}
