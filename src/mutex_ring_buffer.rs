use parking_lot::Mutex;
use std::mem::MaybeUninit;

use crate::primitives::Arc;

#[derive(Debug)]
struct RingBuffer<T, const N: usize> {
    head: usize,
    tail: usize,
    buffer: [MaybeUninit<T>; N],
}

#[derive(Debug, Clone)]
pub struct MutexRingBuffer<T, const N: usize>(Arc<Mutex<RingBuffer<T, N>>>);

impl<T, const N: usize> Default for MutexRingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> MutexRingBuffer<T, N> {
    pub fn new() -> Self {
        const {
            assert!(
                N != 0 && N.is_power_of_two(),
                "Buffer size N must be a power of two"
            )
        };
        Self(Arc::new(Mutex::new(RingBuffer {
            buffer: std::array::from_fn(|_| MaybeUninit::uninit()),
            head: 0,
            tail: 0,
        })))
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let mut ring_buffer = self.0.lock();

        if ring_buffer.head.wrapping_sub(ring_buffer.tail) == N {
            return Err(value);
        }

        let idx = Self::mask(ring_buffer.head);
        unsafe {
            ring_buffer.buffer.get_unchecked_mut(idx).write(value);
        }
        ring_buffer.head = ring_buffer.head.wrapping_add(1);
        Ok(())
    }

    pub fn read(&self) -> Option<T> {
        let mut ring_buffer = self.0.lock();
        if ring_buffer.tail != ring_buffer.head {
            let idx = Self::mask(ring_buffer.tail);
            let value;
            unsafe {
                let ptr = ring_buffer.buffer.get_unchecked(idx).as_ptr();

                value = std::ptr::read(ptr);
            }
            ring_buffer.tail = ring_buffer.tail.wrapping_add(1);
            return Some(value);
        }

        None
    }
    #[inline(always)]
    fn mask(index: usize) -> usize {
        index & (N - 1)
    }
}

impl<T, const N: usize> Drop for RingBuffer<T, N> {
    fn drop(&mut self) {
        if std::mem::needs_drop::<T>() {
            while self.tail != self.head {
                let mask = self.tail & (N - 1);
                unsafe {
                    std::ptr::drop_in_place(self.buffer.get_unchecked_mut(mask).as_mut_ptr());
                }
                self.tail = self.tail.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn test_basic_push_pop_wrap() {
        let buffer = MutexRingBuffer::<i32, 4>::new();

        assert!(buffer.push(1).is_ok());
        assert!(buffer.push(2).is_ok());
        assert!(buffer.push(3).is_ok());
        assert!(buffer.push(4).is_ok());

        assert_eq!(buffer.push(5), Err(5));
        assert_eq!(buffer.read(), Some(1));
        assert_eq!(buffer.read(), Some(2));

        assert!(buffer.push(5).is_ok());
        assert!(buffer.push(6).is_ok());

        assert_eq!(buffer.push(7), Err(7));

        assert_eq!(buffer.read(), Some(3));
        assert_eq!(buffer.read(), Some(4));
        assert_eq!(buffer.read(), Some(5));
        assert_eq!(buffer.read(), Some(6));

        assert_eq!(buffer.read(), None);
    }

    #[test]
    fn test_multithreaded_concurrency() {
        let buffer = MutexRingBuffer::<usize, 32>::new();
        let total_items = 10_000;

        let producer_sum = Arc::new(AtomicUsize::new(0));
        let consumer_sum = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];

        for _ in 0..2 {
            let buf = buffer.clone();
            let sum = producer_sum.clone();
            handles.push(thread::spawn(move || {
                for i in 0..(total_items / 2) {
                    loop {
                        if buf.push(i).is_ok() {
                            sum.fetch_add(i, Ordering::Relaxed);
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        for _ in 0..2 {
            let buf = buffer.clone();
            let sum = consumer_sum.clone();
            handles.push(thread::spawn(move || {
                let mut count = 0;
                while count < (total_items / 2) {
                    if let Some(val) = buf.read() {
                        sum.fetch_add(val, Ordering::Relaxed);
                        count += 1;
                    } else {
                        std::hint::spin_loop();
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            producer_sum.load(Ordering::Relaxed),
            consumer_sum.load(Ordering::Relaxed),
            "Sum of pushed items should equal sum of popped items"
        );
    }

    #[test]
    fn test_drop_cleanup() {
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug)]
        struct Droppable;
        impl Drop for Droppable {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
        }

        DROP_COUNT.store(0, Ordering::Relaxed);

        {
            let buffer = MutexRingBuffer::<Droppable, 8>::new();

            for _ in 0..5 {
                buffer.push(Droppable).unwrap();
            }

            {
                let _a = buffer.read();
                let _b = buffer.read();
            }

            assert_eq!(
                DROP_COUNT.load(Ordering::Relaxed),
                2,
                "Popped items didn't drop"
            );
        }

        assert_eq!(
            DROP_COUNT.load(Ordering::Relaxed),
            5,
            "Buffer failed to drop remaining items"
        );
    }

    #[test]
    fn test_zst() {
        struct Zst;

        let buffer = MutexRingBuffer::<Zst, 4>::new();

        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_ok());
        assert!(buffer.push(Zst).is_err());

        assert!(buffer.read().is_some());
        assert!(buffer.push(Zst).is_ok());
    }
}
