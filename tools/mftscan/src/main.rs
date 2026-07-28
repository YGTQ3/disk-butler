//! mftscan: MFT direct-read prototype (docs/12 stage 2).
//! Compares NTFS $MFT sequential scan vs jwalk directory walk on the same drive:
//! elapsed time, file count, total bytes, top-level dir aggregation, deviation %.
//! Read-only. Requires Administrator for the MFT channel (volume handle \\.\X:).
//! Output: console (ASCII) + mftscan-report.txt in the current directory.

use std::collections::HashMap;
use std::io::Write as _;
use std::time::Instant;

use jwalk::WalkDir;
use ntfs_reader::file_info::{FileInfo, VecCache};
use ntfs_reader::mft::Mft;
use ntfs_reader::volume::Volume;

struct Summary {
    elapsed_secs: f64,
    files: u64,
    bytes: u64,
    /// first-level segment (lowercase) -> total bytes
    top_dirs: HashMap<String, u64>,
}

/// Normalize a path string and return its first segment under the drive root.
/// Works for both "C:\Users\..." (jwalk) and "\\.\C:\Users\..." (MFT) forms.
fn first_level_segment(path_str: &str, drive: char) -> Option<String> {
    let lower = path_str.to_lowercase().replace('/', "\\");
    let pat = format!("{}:\\", drive.to_ascii_lowercase());
    let idx = lower.find(&pat)? + pat.len();
    let rest = &lower[idx..];
    let seg = rest.split('\\').next().unwrap_or("");
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

/// Skip NTFS metafiles and the recycle bin on BOTH channels so the comparison
/// baseline is identical ($MFT/$LogFile alone would blow the +/-2% line).
fn skipped(seg: &str) -> bool {
    seg.starts_with('$')
}

fn scan_mft(drive: char) -> Result<Summary, String> {
    let t = Instant::now();
    let volume = Volume::new(format!("\\\\.\\{}:", drive))
        .map_err(|e| format!("open volume \\\\.\\{}: failed: {:?} (administrator required)", drive, e))?;
    let mft = Mft::new(volume).map_err(|e| format!("read $MFT failed: {:?}", e))?;
    let mut cache = VecCache::default();
    let mut files: u64 = 0;
    let mut bytes: u64 = 0;
    let mut top_dirs: HashMap<String, u64> = HashMap::new();
    for file in mft.files() {
        let info = FileInfo::with_cache(&mft, &file, &mut cache);
        if info.is_directory {
            continue;
        }
        let ps = info.path.to_string_lossy();
        let Some(seg) = first_level_segment(&ps, drive) else { continue };
        if skipped(&seg) {
            continue;
        }
        files += 1;
        bytes += info.size;
        *top_dirs.entry(seg).or_insert(0) += info.size;
    }
    Ok(Summary {
        elapsed_secs: t.elapsed().as_secs_f64(),
        files,
        bytes,
        top_dirs,
    })
}

fn scan_jwalk(drive: char) -> Summary {
    let t = Instant::now();
    let root = format!("{}:\\", drive);
    let mut files: u64 = 0;
    let mut bytes: u64 = 0;
    let mut top_dirs: HashMap<String, u64> = HashMap::new();
    for entry in WalkDir::new(&root)
        .skip_hidden(false)
        .parallelism(jwalk::Parallelism::RayonDefaultPool {
            busy_timeout: std::time::Duration::from_secs(5),
        })
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let ps = entry.path().to_string_lossy().to_string();
        let Some(seg) = first_level_segment(&ps, drive) else { continue };
        if skipped(&seg) {
            continue;
        }
        files += 1;
        bytes += size;
        *top_dirs.entry(seg).or_insert(0) += size;
    }
    Summary {
        elapsed_secs: t.elapsed().as_secs_f64(),
        files,
        bytes,
        top_dirs,
    }
}

fn gb(v: u64) -> f64 {
    v as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn delta_pct(a: u64, b: u64) -> f64 {
    if b == 0 {
        return 0.0;
    }
    (a as f64 - b as f64) / b as f64 * 100.0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut drive = 'C';
    let mut mode = "both".to_string();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mode" => {
                if let Some(m) = it.next() {
                    mode = m.to_lowercase();
                }
            }
            s if s.len() == 1 || (s.len() == 2 && s.ends_with(':')) => {
                drive = s.chars().next().unwrap().to_ascii_uppercase();
            }
            _ => {
                eprintln!("usage: mftscan [drive] [--mode mft|jwalk|both]");
                std::process::exit(2);
            }
        }
    }

    let mut out = String::new();
    out.push_str(&format!("=== mftscan prototype: drive {}:, mode {} ===\n", drive, mode));
    out.push_str("(NTFS metafiles and $Recycle.Bin excluded on both channels)\n\n");

    // Run MFT first so the jwalk pass benefits from a warmed OS cache if anything
    // (bias favors jwalk, making the speedup claim conservative).
    let mft_sum = if mode == "mft" || mode == "both" {
        match scan_mft(drive) {
            Ok(s) => {
                out.push_str(&format!(
                    "[MFT]   elapsed {:>8.2} s  files {:>9}  bytes {:>15} ({:.2} GB)\n",
                    s.elapsed_secs, s.files, s.bytes, gb(s.bytes)
                ));
                Some(s)
            }
            Err(e) => {
                out.push_str(&format!("[MFT]   FAILED: {}\n", e));
                None
            }
        }
    } else {
        None
    };

    let jwalk_sum = if mode == "jwalk" || mode == "both" {
        let s = scan_jwalk(drive);
        out.push_str(&format!(
            "[jwalk] elapsed {:>8.2} s  files {:>9}  bytes {:>15} ({:.2} GB)\n",
            s.elapsed_secs, s.files, s.bytes, gb(s.bytes)
        ));
        Some(s)
    } else {
        None
    };

    if let (Some(m), Some(j)) = (&mft_sum, &jwalk_sum) {
        out.push('\n');
        if m.elapsed_secs > 0.0 {
            out.push_str(&format!("speedup: {:.1}x\n", j.elapsed_secs / m.elapsed_secs));
        }
        let fd = delta_pct(m.files, j.files);
        let bd = delta_pct(m.bytes, j.bytes);
        out.push_str(&format!(
            "file count delta (MFT vs jwalk): {:+.2}%   bytes delta: {:+.2}%   (acceptance: +/-2%)\n",
            fd, bd
        ));

        // Top 20 first-level dirs by MFT bytes, with per-dir deviation.
        let mut tops: Vec<(&String, &u64)> = m.top_dirs.iter().collect();
        tops.sort_by(|a, b| b.1.cmp(a.1));
        out.push_str("\nTop 20 first-level dirs (by MFT bytes):\n");
        out.push_str(&format!("{:<40} {:>10} {:>10} {:>9}\n", "dir", "mft(GB)", "jwalk(GB)", "delta%"));
        for (seg, mb) in tops.iter().take(20) {
            let jb = j.top_dirs.get(*seg).copied().unwrap_or(0);
            out.push_str(&format!(
                "{:<40} {:>10.2} {:>10.2} {:>+8.2}%\n",
                seg,
                gb(**mb),
                gb(jb),
                delta_pct(**mb, jb)
            ));
        }

        let pass = fd.abs() <= 2.0 && bd.abs() <= 2.0;
        out.push_str(&format!(
            "\nverdict: {} (docs/12 acceptance line: total deviation within +/-2%)\n",
            if pass { "PASS" } else { "FAIL" }
        ));
    }

    print!("{}", out);
    if let Ok(mut f) = std::fs::File::create("mftscan-report.txt") {
        let _ = f.write_all(out.as_bytes());
        println!("\nreport saved to mftscan-report.txt");
    }
}
