use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::Instant;

struct MockPlaybackState {
    position_ms: u64,
    duration_ms: u64,
    volume: f32,
    playing: bool,
}

impl MockPlaybackState {
    fn new() -> Self {
        Self {
            position_ms: 0,
            duration_ms: 240_000,
            volume: 0.8,
            playing: false,
        }
    }

    fn play(&mut self) {
        self.playing = true;
    }

    fn pause(&mut self) {
        self.playing = false;
    }

    fn seek(&mut self, position_ms: u64) {
        self.position_ms = position_ms.min(self.duration_ms);
    }

    fn tick(&mut self, elapsed_ms: u64) {
        if self.playing {
            self.position_ms = (self.position_ms + elapsed_ms).min(self.duration_ms);
        }
    }

    fn volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
    }
}

fn bench_playback_commands(c: &mut Criterion) {
    c.bench_function("playback_state_transitions", |b| {
        let mut state = MockPlaybackState::new();
        b.iter(|| {
            black_box(state.play());
            black_box(state.tick(50));
            black_box(state.pause());
            black_box(state.seek(1000));
            black_box(state.tick(100));
            black_box(state.volume(0.5));
            black_box(state.play());
        })
    });
}

fn bench_seek_latency(c: &mut Criterion) {
    c.bench_function("playback_seek_overhead", |b| {
        let mut state = MockPlaybackState::new();
        state.play();
        b.iter(|| {
            black_box(state.seek(black_box(50_000)));
            black_box(state.tick(1));
        })
    });
}

fn bench_volume_change(c: &mut Criterion) {
    c.bench_function("playback_volume_change", |b| {
        let mut state = MockPlaybackState::new();
        b.iter(|| {
            for vol in 0..=100 {
                black_box(state.volume(black_box(vol as f32 / 100.0)));
            }
        })
    });
}

fn bench_position_tracking(c: &mut Criterion) {
    c.bench_function("playback_position_tracking", |b| {
        let mut state = MockPlaybackState::new();
        state.play();
        b.iter(|| {
            let start = Instant::now();
            black_box(state.tick(1));
            black_box(state.position_ms);
            let _elapsed = start.elapsed();
        })
    });
}

criterion_group!(
    playback_benches,
    bench_playback_commands,
    bench_seek_latency,
    bench_volume_change,
    bench_position_tracking,
);
criterion_main!(playback_benches);
