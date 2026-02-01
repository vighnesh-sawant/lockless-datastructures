use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::primitives::Arc;
use crate::{Backoff, Padded};

#[repr(align(64))]
struct Slot<T> {
    sequence: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

///Uses atomic's instead of mutexes
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

    pub fn pop(&self) -> Option<T> {
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
    pub fn read_head(&self) -> usize {
        self.head.load(Ordering::Acquire) % N
    }

    pub fn read_tail(&self) -> usize {
        self.tail.load(Ordering::Acquire) % N
    }

    pub fn exists(&self, index: usize) -> bool {
        let mut tail = self.tail.load(Ordering::Acquire);
        let mut head = self.head.load(Ordering::Acquire);
        if head == tail {
            return false;
        }
        head &= N - 1;
        tail &= N - 1;
        if head > tail {
            head > index && index >= tail
        } else {
            !(index >= head && tail > index)
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
                    let raw_ptr = (*slot.data.get()).as_mut_ptr();
                    std::ptr::drop_in_place(raw_ptr);
                }
            }

            tail = tail.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn test_basic_push_and_read() {
        let queue: Arc<AtomicRingBufferMpmc<i32, 4>> = AtomicRingBufferMpmc::new();

        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert!(queue.push(3).is_ok());

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_buffer_full() {
        let queue: Arc<AtomicRingBufferMpmc<i32, 2>> = AtomicRingBufferMpmc::new();

        assert!(queue.push(10).is_ok());
        assert!(queue.push(20).is_ok());

        let result = queue.push(30);
        assert_eq!(result, Err(30));

        assert_eq!(queue.pop(), Some(10));

        assert!(queue.push(30).is_ok());
        assert_eq!(queue.pop(), Some(20));
        assert_eq!(queue.pop(), Some(30));
    }

    #[test]
    fn test_wrap_around() {
        let queue: Arc<AtomicRingBufferMpmc<usize, 4>> = AtomicRingBufferMpmc::new();

        for i in 0..100 {
            assert!(queue.push(i).is_ok());
            assert_eq!(queue.pop(), Some(i));
        }

        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_mpmc_concurrency() {
        const BUFFER_SIZE: usize = 64;
        const NUM_PRODUCERS: usize = 4;
        const NUM_CONSUMERS: usize = 4;
        const OPS_PER_THREAD: usize = 10_000;

        let queue: Arc<AtomicRingBufferMpmc<usize, BUFFER_SIZE>> = AtomicRingBufferMpmc::new();
        let barrier = Arc::new(Barrier::new(NUM_PRODUCERS + NUM_CONSUMERS));

        let mut handles = vec![];

        for p_id in 0..NUM_PRODUCERS {
            let q = queue.clone();
            let b = barrier.clone();
            handles.push(thread::spawn(move || {
                b.wait();
                for i in 0..OPS_PER_THREAD {
                    let value = p_id * OPS_PER_THREAD + i;
                    while q.push(value).is_err() {
                        std::thread::yield_now();
                    }
                }
            }));
        }

        let results = Arc::new(AtomicUsize::new(0));
        for _ in 0..NUM_CONSUMERS {
            let q = queue.clone();
            let b = barrier.clone();
            let r = results.clone();
            handles.push(thread::spawn(move || {
                b.wait();

                loop {
                    match q.pop() {
                        Some(_) => {
                            r.fetch_add(1, Ordering::Relaxed);
                        }
                        None => {
                            if r.load(Ordering::Relaxed) == NUM_PRODUCERS * OPS_PER_THREAD {
                                break;
                            }
                            std::thread::yield_now();
                        }
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            results.load(Ordering::SeqCst),
            NUM_PRODUCERS * OPS_PER_THREAD,
            "Total items consumed must match total items produced"
        );
    }
    static DROP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug)]
    struct DropTracker;

    impl Drop for DropTracker {
        fn drop(&mut self) {
            DROP_COUNTER.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_drop_cleanup() {
        DROP_COUNTER.store(0, Ordering::Relaxed);

        {
            let buffer = AtomicRingBufferMpmc::<DropTracker, 8>::new();

            for _ in 0..5 {
                buffer.push(DropTracker).unwrap();
            }

            buffer.pop();
            buffer.pop();

            assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 2);
        }

        assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 5);
    }
}
