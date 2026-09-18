# strictdu

A disk usage scanner that refuses to give you a confident-looking number
built on data it couldn't actually read.

## The problem

Run `du` on a directory tree that has a permission-denied subdirectory or
a broken symlink somewhere in it, and you'll get a total printed to
stdout next to a pile of "Permission denied" lines on stderr. The total
is silently short by whatever it couldn't read. That's fine at an
interactive terminal where you'll notice the red text, but it's a trap
in a script, a cron job, or a CI log nobody reads until something breaks:
the number gets piped into a threshold check or a report and nobody
finds out it was wrong until the disk that "had 40GB free" is full.

strictdu inverts the default: any entry it can't read aborts the scan
with a non-zero exit and an error naming the exact path, unless you pass
`--lenient`, in which case it behaves like traditional `du` — skip what
it can't read, keep going, and tell you what it skipped.

## Usage

```
$ strictdu ~/Downloads
  842.1 MiB  /home/me/Downloads/vm-images
  120.4 MiB  /home/me/Downloads/conference-talk.mkv
    3.2 MiB  /home/me/Downloads/invoice.pdf
  965.7 MiB  total (/home/me/Downloads)
```

If something under the tree can't be read:

```
$ strictdu /var/log
strictdu: /var/log/private-app: Permission denied (os error 13)
strictdu: pass --lenient to skip unreadable entries instead of aborting
$ echo $?
1
```

```
$ strictdu --lenient /var/log
   88.0 KiB  /var/log/app.log
    1.2 MiB  total (/var/log)

1 item(s) skipped (--lenient):
  /var/log/private-app: Permission denied (os error 13)
```

## As a library

```rust
let options = strictdu::ScanOptions { lenient: false };
let report = strictdu::scan(std::path::Path::new("."), options)?;

for entry in &report.entries {
    println!("{} bytes  {}", entry.bytes, entry.path.display());
}
println!("total: {} bytes", report.total_bytes);
```

`scan` returns a `Result<ScanReport, ScanError>`. In lenient mode it
practically never errors — failures are folded into `report.warnings`
instead so you get a best-effort total plus a record of what was
skipped.

## What "size" means here

Sizes are the apparent size of regular files (`len()` from `stat`), not
blocks allocated on disk, and symlinks are never followed — a symlink
itself is treated as a special file, not traversed into. Directories
contribute the sum of what's under them, not their own inode size.

## Status

Early skeleton. Single-threaded, no exclude patterns, no depth limit,
no machine-readable output yet.

## License

MIT, see [LICENSE](LICENSE).
