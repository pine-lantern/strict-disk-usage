use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut lenient = false;
    let mut path: Option<String> = None;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--lenient" => lenient = true,
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other => {
                if path.is_some() {
                    eprintln!("strictdu: unexpected extra argument '{other}'");
                    return ExitCode::from(2);
                }
                path = Some(other.to_string());
            }
        }
    }

    let root = PathBuf::from(path.unwrap_or_else(|| ".".to_string()));
    let options = strictdu::ScanOptions { lenient };

    match strictdu::scan(&root, options) {
        Ok(report) => {
            for entry in &report.entries {
                println!("{:>10}  {}", human_size(entry.bytes), entry.path.display());
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
    println!("strictdu [--lenient] [PATH]");
    println!();
    println!("Report the size of each direct child of PATH, largest first.");
    println!("Defaults PATH to the current directory.");
    println!();
    println!("By default, any unreadable file, denied permission, or non-regular");
    println!("entry (broken symlink, socket, device file) aborts the scan with an");
    println!("error instead of quietly leaving it out of the total.");
    println!();
    println!("  --lenient   skip unreadable entries and print a warning summary");
    println!("              on stderr instead of aborting");
    println!("  -h, --help  print this message");
}
