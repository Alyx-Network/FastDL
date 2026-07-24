use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use bzip2::write::BzEncoder;
use bzip2::Compression as BzCompression;
use flate2::write::GzEncoder;
use flate2::Compression as GzCompression;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

const COMPRESSIONS: [&str; 2] = [".bz2", ".gz"];

pub struct Summary {
    pub files: usize,
    pub created: usize,
    pub skipped: usize,
    pub elapsed_ms: u128,
}

pub fn generate_all(storage_root: &Path, auto_root: &Path) -> io::Result<Summary> {
    let started = Instant::now();
    let mut sources = Vec::new();
    collect_files(storage_root, &mut sources)?;
    let mut created = 0;
    let mut skipped = 0;
    for source in &sources {
        if is_archive(source) {
            continue;
        }
        let relative = match source.strip_prefix(storage_root) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        for compression in COMPRESSIONS {
            let target = append_extension(auto_root.join(relative), compression);
            match up_to_date(source, &target)? {
                true => skipped += 1,
                false => {
                    compress(source, &target, compression)?;
                    created += 1;
                }
            }
        }
    }
    Ok(Summary {
        files: sources.len(),
        created,
        skipped,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

pub fn watch(storage_root: PathBuf, auto_root: PathBuf) -> notify::Result<RecommendedWatcher> {
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| match event {
        Ok(change) => match change.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                let _ = sender.send(());
            }
            _ => {}
        },
        Err(error) => tracing::warn!(error = %error, "Storage watcher error [storage_watch_failed]"),
    })?;
    watcher.watch(&storage_root, RecursiveMode::Recursive)?;
    thread::spawn(move || {
        for _ in receiver.iter() {
            while receiver.try_recv().is_ok() {}
            match generate_all(&storage_root, &auto_root) {
                Ok(summary) => tracing::info!(files = summary.files, created = summary.created, skipped = summary.skipped, elapsed_ms = summary.elapsed_ms, "Regenerated archives [archives_regenerated]"),
                Err(error) => tracing::warn!(error = %error, "Failed to regenerate archives [archives_regen_failed]"),
            }
        }
    });
    Ok(watcher)
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        match path.is_dir() {
            true => collect_files(&path, files)?,
            false => files.push(path),
        }
    }
    Ok(())
}

fn is_archive(path: &Path) -> bool {
    match path.extension().and_then(|value| value.to_str()) {
        Some("bz2") | Some("gz") => true,
        _ => false,
    }
}

fn up_to_date(source: &Path, target: &Path) -> io::Result<bool> {
    let target_metadata = match std::fs::metadata(target) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(false),
    };
    Ok(target_metadata.modified()? >= std::fs::metadata(source)?.modified()?)
}

fn compress(source: &Path, target: &Path, compression: &str) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = append_extension(target.to_path_buf(), ".part");
    match compression {
        ".bz2" => write_bzip2(source, &temporary)?,
        _ => write_gzip(source, &temporary)?,
    }
    std::fs::rename(&temporary, target)?;
    Ok(())
}

fn write_bzip2(source: &Path, target: &Path) -> io::Result<()> {
    let mut encoder = BzEncoder::new(std::fs::File::create(target)?, BzCompression::best());
    copy_into(source, &mut encoder)?;
    encoder.finish()?;
    Ok(())
}

fn write_gzip(source: &Path, target: &Path) -> io::Result<()> {
    let mut encoder = GzEncoder::new(std::fs::File::create(target)?, GzCompression::best());
    copy_into(source, &mut encoder)?;
    encoder.finish()?;
    Ok(())
}

fn copy_into<W: Write>(source: &Path, encoder: &mut W) -> io::Result<()> {
    io::copy(&mut BufReader::new(std::fs::File::open(source)?), encoder)?;
    Ok(())
}

fn append_extension(path: PathBuf, extension: &str) -> PathBuf {
    let mut value = path.into_os_string();
    value.push(extension);
    PathBuf::from(value)
}
