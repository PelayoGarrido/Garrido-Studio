use crate::{Error, Result};
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

struct Shared {
    frames: Box<[[AtomicU32; 2]]>,
    read: AtomicU64,
    write: AtomicU64,
}
pub struct Producer(Arc<Shared>);
pub struct Consumer(Arc<Shared>);

pub fn pipe(capacity: usize) -> Result<(Producer, Consumer)> {
    if !(64..=65536).contains(&capacity) || !capacity.is_power_of_two() {
        return Err(Error::Invalid("Capacidad de audio no válida"));
    }
    let shared = Arc::new(Shared {
        frames: (0..capacity)
            .map(|_| [AtomicU32::new(0), AtomicU32::new(0)])
            .collect(),
        read: AtomicU64::new(0),
        write: AtomicU64::new(0),
    });
    Ok((Producer(shared.clone()), Consumer(shared)))
}
impl Producer {
    pub fn available(&self) -> usize {
        let write = self.0.write.load(Ordering::Relaxed);
        let read = self.0.read.load(Ordering::Acquire);
        self.0
            .frames
            .len()
            .saturating_sub(write.wrapping_sub(read) as usize)
    }
    pub fn push(&mut self, frames: &[[f32; 2]]) -> usize {
        let count = frames.len().min(self.available());
        let write = self.0.write.load(Ordering::Relaxed);
        let mask = self.0.frames.len() - 1;
        for (i, frame) in frames.iter().take(count).enumerate() {
            let destination = &self.0.frames[write.wrapping_add(i as u64) as usize & mask];
            for ch in 0..2 {
                destination[ch].store(frame[ch].to_bits(), Ordering::Relaxed);
            }
        }
        self.0
            .write
            .store(write.wrapping_add(count as u64), Ordering::Release);
        count
    }
}
impl Consumer {
    pub fn available(&self) -> usize {
        let read = self.0.read.load(Ordering::Relaxed);
        let write = self.0.write.load(Ordering::Acquire);
        (write.wrapping_sub(read) as usize).min(self.0.frames.len())
    }
    pub fn pop(&mut self, frames: &mut [[f32; 2]]) -> usize {
        self.pop_bounded(frames, 0.8)
    }
    pub fn pop_capture(&mut self, frames: &mut [[f32; 2]]) -> usize {
        self.pop_bounded(frames, 1.)
    }
    fn pop_bounded(&mut self, frames: &mut [[f32; 2]], ceiling: f32) -> usize {
        let read = self.0.read.load(Ordering::Relaxed);
        let write = self.0.write.load(Ordering::Acquire);
        let count = frames
            .len()
            .min(write.wrapping_sub(read) as usize)
            .min(self.0.frames.len());
        let mask = self.0.frames.len() - 1;
        for (i, frame) in frames.iter_mut().take(count).enumerate() {
            let source = &self.0.frames[read.wrapping_add(i as u64) as usize & mask];
            *frame = source.each_ref().map(|v| {
                let value = f32::from_bits(v.load(Ordering::Relaxed));
                if value.is_finite() {
                    value.clamp(-ceiling, ceiling)
                } else {
                    0.
                }
            });
        }
        frames[count..].fill([0.; 2]);
        self.0
            .read
            .store(read.wrapping_add(count as u64), Ordering::Release);
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraparound_nonfinite_audio_and_backpressure_are_bounded() {
        let (mut tx, mut rx) = pipe(64).unwrap();
        tx.0.read.store(u64::MAX - 31, Ordering::Relaxed);
        tx.0.write.store(u64::MAX - 31, Ordering::Relaxed);
        assert_eq!(tx.push(&[[f32::NAN, 2.]; 80]), 64);
        assert_eq!(tx.available(), 0);
        assert_eq!(tx.push(&[[1.; 2]]), 0);
        let mut out = [[9.; 2]; 80];
        assert_eq!(rx.pop(&mut out), 64);
        assert_eq!(out[0], [0., 0.8]);
        assert_eq!(out[63], [0., 0.8]);
        assert_eq!(out[64], [0.; 2]);
        assert_eq!(tx.available(), 64);
        assert_eq!(rx.pop(&mut out), 0);
        assert_eq!(out, [[0.; 2]; 80]);
        assert!(pipe(63).is_err());
        assert!(pipe(1000).is_err());
        assert!(pipe(65537).is_err());
    }
}
