use std::ops::{Deref, DerefMut};

mod atomic_ring_buffer_mpmc;
mod atomic_ring_buffer_spsc;
mod mutex_ring_buffer;
mod primitives;
mod render;

pub use self::atomic_ring_buffer_mpmc::AtomicRingBufferMpmc;
pub use self::atomic_ring_buffer_spsc::AtomicRingBufferSpsc;
pub use self::mutex_ring_buffer::MutexRingBuffer;

///Use to prevent cache line collision!
#[derive(Debug, Default)]
#[repr(align(64))]
pub struct Padded<T>(pub T);
impl<T> Deref for Padded<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Padded<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

use std::hint;
use std::thread;
///An exponential backoff
pub struct Backoff {
    step: u32,
}
impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}
impl Backoff {
    pub fn new() -> Self {
        Self { step: 0 }
    }
    ///Call this where you want to backoff!
    #[inline]
    pub fn snooze(&mut self) {
        if self.step <= 6 {
            for _ in 0..(1 << self.step) {
                hint::spin_loop();
            }
        } else {
            thread::yield_now();
        }

        if self.step <= 6 {
            self.step += 1;
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.step = 0;
    }
}
