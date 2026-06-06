use std::collections::VecDeque;
use std::num::NonZero;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::Source;

const RING_BUFFER_CAPACITY: usize = 16384;
const TAP_BATCH_SIZE: usize = 256;

pub struct AudioRingBuffer {
    inner: Mutex<RingState>,
}

struct RingState {
    buffer: VecDeque<f32>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
}

impl AudioRingBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(RingState {
                buffer: VecDeque::with_capacity(RING_BUFFER_CAPACITY),
                sample_rate: None,
                channels: None,
            }),
        })
    }

    pub fn push_batch(&self, samples: &[f32]) {
        let mut state = self.inner.lock().unwrap();
        for &s in samples {
            if state.buffer.len() >= RING_BUFFER_CAPACITY {
                state.buffer.pop_front();
            }
            state.buffer.push_back(s);
        }
    }

    pub fn set_format(&self, sample_rate: u32, channels: u16) {
        let mut state = self.inner.lock().unwrap();
        state.sample_rate = Some(sample_rate);
        state.channels = Some(channels);
    }

    pub fn drain(&self, max_count: usize) -> Vec<f32> {
        let mut state = self.inner.lock().unwrap();
        let n = max_count.min(state.buffer.len());
        state.buffer.drain(..n).collect()
    }

    pub fn sample_rate(&self) -> Option<u32> {
        self.inner.lock().unwrap().sample_rate
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().buffer.len()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().buffer.clear();
    }
}

pub struct SpectrumTap {
    inner: Box<dyn Source<Item = f32> + Send>,
    ring: Arc<AudioRingBuffer>,
    local_buf: Vec<f32>,
}

impl SpectrumTap {
    pub fn new(inner: Box<dyn Source<Item = f32> + Send>, ring: Arc<AudioRingBuffer>) -> Self {
        let sample_rate = inner.sample_rate().get();
        let channels = inner.channels().get();
        ring.set_format(sample_rate, channels);

        Self {
            inner,
            ring,
            local_buf: Vec::with_capacity(TAP_BATCH_SIZE),
        }
    }
}

impl Iterator for SpectrumTap {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        self.local_buf.push(sample);
        if self.local_buf.len() >= TAP_BATCH_SIZE {
            self.ring.push_batch(&self.local_buf);
            self.local_buf.clear();
        }
        Some(sample)
    }
}

impl Source for SpectrumTap {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> NonZero<u16> {
        self.inner.channels()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.local_buf.clear();
        self.inner.try_seek(pos)
    }
}

impl Drop for SpectrumTap {
    fn drop(&mut self) {
        if !self.local_buf.is_empty() {
            self.ring.push_batch(&self.local_buf);
        }
    }
}
