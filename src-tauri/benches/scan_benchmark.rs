use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "aac", "aif", "aiff", "alac", "flac", "m4a", "mp3", "ogg", "opus", "wav",
];

fn create_fake_library(root: &PathBuf, file_count: usize) {
    for i in 0..file_count {
        let ext = AUDIO_EXTENSIONS[i % AUDIO_EXTENSIONS.len()];
        let dir = root
            .join(format!("artist_{}", i % 20))
            .join(format!("album_{}", i % 50));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("track_{}.{ext}", i)),
            b"fake audio file content for benchmarking",
        )
        .unwrap();
    }
}

fn scan_directory(path: &PathBuf) -> usize {
    let mut count = 0;
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn bench_scan_small(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().to_path_buf();
    create_fake_library(&root, 100);

    c.bench_function("scan_library_100_files", |b| {
        b.iter(|| {
            let count = scan_directory(black_box(&root));
            black_box(count);
        })
    });
}

fn bench_scan_medium(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().to_path_buf();
    create_fake_library(&root, 1_000);

    c.bench_function("scan_library_1000_files", |b| {
        b.iter(|| {
            let count = scan_directory(black_box(&root));
            black_box(count);
        })
    });
}

fn bench_scan_large(c: &mut Criterion) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().to_path_buf();
    create_fake_library(&root, 5_000);

    c.bench_function("scan_library_5000_files", |b| {
        b.iter(|| {
            let count = scan_directory(black_box(&root));
            black_box(count);
        })
    });
}

criterion_group!(
    scan_benches,
    bench_scan_small,
    bench_scan_medium,
    bench_scan_large
);
criterion_main!(scan_benches);
