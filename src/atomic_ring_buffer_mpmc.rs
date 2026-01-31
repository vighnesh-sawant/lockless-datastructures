use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{Backoff, Padded};

#[repr(align(64))]
struct Slot<T> {
    sequence: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

pub struct AtomicRingBufferMpmc<T, const N: usize> {
    head: Padded<AtomicUsize>,
    tail: Padded<AtomicUsize>,
    buffer: [Slot<T>; N],
}

unsafe impl<T: Send, const N: usize> Sync for AtomicRingBufferMpmc<T, N> {}
unsafe impl<T: Send, const N: usize> Send for AtomicRingBufferMpmc<T, N> {}

impl<T, const N: usize> AtomicRingBufferMpmc<T, N> {
    pub fn new() -> Arc<Self> {
        const { assert!(N != 0 && N.is_power_of_two()) };

        let buffer = std::array::from_fn(|i| Slot {
            sequence: AtomicUsize::new(i),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        });

        Arc::new(Self {
            head: Padded(AtomicUsize::new(0)),
            tail: Padded(AtomicUsize::new(0)),
            buffer,
        })
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let mut backoff = Backoff::new();
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            let idx = head & (N - 1);
            let slot;
            unsafe {
                slot = self.buffer.get_unchecked(idx);
            }
            let seq = slot.sequence.load(Ordering::Acquire);

            let diff = seq as isize - head as isize;

            if diff == 0 {
                match self.head.compare_exchange_weak(
                    head,
                    head + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        unsafe {
                            (*slot.data.get()).write(value);
                        }
                        slot.sequence.store(head.wrapping_add(1), Ordering::Release);
                        return Ok(());
                    }
                    Err(real_head) => {
                        head = real_head;
                    }
                }
            } else if diff < 0 {
                let new_head = self.head.load(Ordering::Relaxed);
                if new_head != head {
                    head = new_head;
                    backoff.reset();
                    continue;
                }
                return Err(value);
            } else {
                head = self.head.load(Ordering::Relaxed);
            }

            backoff.snooze();
        }
    }

    pub fn read(&self) -> Option<T> {
        let mut backoff = Backoff::new();
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            let idx = tail & (N - 1);
            let slot;
            unsafe {
                slot = self.buffer.get_unchecked(idx);
            }

            let seq = slot.sequence.load(Ordering::Acquire);

            let diff = seq as isize - (tail.wrapping_add(1) as isize);

            if diff == 0 {
                match self.tail.compare_exchange_weak(
                    tail,
                    tail + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let value = unsafe { (*slot.data.get()).assume_init_read() };

                        slot.sequence.store(tail.wrapping_add(N), Ordering::Release);

                        return Some(value);
                    }
                    Err(real_tail) => {
                        tail = real_tail;
                    }
                }
            } else if diff < 0 {
                return None;
            } else {
                tail = self.tail.load(Ordering::Relaxed);
            }

            backoff.snooze();
        }
    }
}

impl<T, const N: usize> Drop for AtomicRingBufferMpmc<T, N> {
    fn drop(&mut self) {
        if !std::mem::needs_drop::<T>() {
            return;
        }

        let head = self.head.load(Ordering::Relaxed);
        let mut tail = self.tail.load(Ordering::Relaxed);

        while tail != head {
            let idx = tail & (N - 1);
            let slot = &self.buffer[idx];

            let seq = slot.sequence.load(Ordering::Relaxed);
            let expected_seq = tail.wrapping_add(1);

            if seq == expected_seq {
                unsafe {
                    let raw_ptr = slot.data.get();
                    std::ptr::drop_in_place(raw_ptr);
                }
            }

            tail = tail.wrapping_add(1);
        }
    }
}
