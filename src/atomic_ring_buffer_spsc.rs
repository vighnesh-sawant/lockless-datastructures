use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::Padded;

#[derive(Debug)]
pub struct AtomicRingBufferSpsc<T, const N: usize> {
    cached_head: UnsafeCell<usize>,
    cached_tail: UnsafeCell<usize>,
    head: Padded<AtomicUsize>,
    tail: Padded<AtomicUsize>,
    buffer: UnsafeCell<[MaybeUninit<T>; N]>,
}
unsafe impl<T, const N: usize> Sync for AtomicRingBufferSpsc<T, N> {}

impl<T, const N: usize> AtomicRingBufferSpsc<T, N> {
    pub fn new() -> Arc<Self> {
        const {
            assert!(
                N != 0 && N.is_power_of_two(),
                "Buffer size N must be a power of two"
            )
        };
        Arc::new(Self {
            cached_head: UnsafeCell::new(0),
            cached_tail: UnsafeCell::new(0),
            buffer: UnsafeCell::new(std::array::from_fn(|_| MaybeUninit::uninit())),
            head: Padded(AtomicUsize::new(0)),
            tail: Padded(AtomicUsize::new(0)),
        })
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let mut tail;
        unsafe {
            tail = self.cached_tail.get().read();
        }

        if head.wrapping_sub(tail) == N {
            tail = self.tail.load(Ordering::Acquire);

            unsafe {
                self.cached_tail.get().write(tail);
            }

            if head.wrapping_sub(tail) == N {
                return Err(value);
            }
        }

        unsafe {
            let buffer_ptr = self.buffer.get();
            let slot = (*buffer_ptr).get_unchecked_mut(head & (N - 1));
            slot.write(value);
        }

        self.head.store(head.wrapping_add(1), Ordering::Release);

        Ok(())
    }

    pub fn read(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);

        let mut head;
        unsafe {
            head = self.cached_head.get().read();
        }

        if tail == head {
            head = self.head.load(Ordering::Acquire);

            unsafe {
                self.cached_head.get().write(head);
            }

            if head == tail {
                return None;
            }
        }

        let value;
        unsafe {
            let buffer_ptr = self.buffer.get();
            let slot = (*buffer_ptr).get_unchecked(tail & (N - 1));
            value = slot.assume_init_read();
        }

        self.tail.store(tail.wrapping_add(1), Ordering::Release);

        Some(value)
    }
}

impl<T, const N: usize> Drop for AtomicRingBufferSpsc<T, N> {
    fn drop(&mut self) {
        if std::mem::needs_drop::<T>() {
            let head = self.head.load(Ordering::Relaxed);
            let tail = self.tail.load(Ordering::Relaxed);

            let mut current = tail;
            while current != head {
                let mask = current & (N - 1);
                unsafe {
                    let slot = (*self.buffer.get()).get_unchecked_mut(mask);
                    std::ptr::drop_in_place(slot.as_mut_ptr());
                }
                current = current.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn test_simple_push_pop() {
        let buffer = AtomicRingBufferSpsc::<i32, 4>::new();

        assert!(buffer.push(1).is_ok());
        assert!(buffer.push(2).is_ok());
        assert!(buffer.push(3).is_ok());
        assert!(buffer.push(4).is_ok());

        assert!(buffer.push(5).is_err());

        assert_eq!(buffer.read(), Some(1));
        assert_eq!(buffer.read(), Some(2));

        assert!(buffer.push(5).is_ok());

        assert_eq!(buffer.read(), Some(3));
        assert_eq!(buffer.read(), Some(4));
        assert_eq!(buffer.read(), Some(5));
        assert_eq!(buffer.read(), None);
    }

    #[test]
    fn test_threaded_spsc_ordering() {
        let buffer = AtomicRingBufferSpsc::<usize, 16>::new();
        let consumer_buffer = buffer.clone();

        let thread_count = 100_000;

        let producer = thread::spawn(move || {
            for i in 0..thread_count {
                while buffer.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        });

        let consumer = thread::spawn(move || {
            for i in 0..thread_count {
                loop {
                    if let Some(val) = consumer_buffer.read() {
                        assert_eq!(val, i, "Items received out of order!");
                        break;
                    }
                    std::hint::spin_loop();
                }
            }
        });

        producer.join().unwrap();
        consumer.join().unwrap();
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
            let buffer = AtomicRingBufferSpsc::<DropTracker, 8>::new();

            for _ in 0..5 {
                buffer.push(DropTracker).unwrap();
            }

            buffer.read();
            buffer.read();

            assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 2);
        }

        assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn test_zst() {
        struct Zst;

        let buffer = AtomicRingBufferSpsc::<Zst, 4>::new();

        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_err());

        assert!(buffer.read().is_some());
        assert!(buffer.push(Zst).is_ok());
    }
}
