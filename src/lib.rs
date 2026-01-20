use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct RingBuffer<T, const N: usize> {
    buffer: [T; N],
    head: usize,
    tail: usize,
}

#[derive(Debug, Clone)]
pub struct MutexRingBuffer<T, const N: usize>(Arc<Mutex<RingBuffer<T, N>>>);

impl<T: Default + Copy, const N: usize> Default for MutexRingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default + Copy, const N: usize> MutexRingBuffer<T, N> {
    pub fn new() -> Self {
        const {
            assert!(
                N != 0 && N.is_power_of_two(),
                "Buffer size N must be a power of two"
            )
        };
        Self(Arc::new(Mutex::new(RingBuffer {
            buffer: std::array::from_fn(|_| T::default()),
            head: 0,
            tail: 0,
        })))
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let mut ring_buffer = match self.0.lock() {
            Ok(lock) => lock,
            Err(_) => return Err(value),
        };

        if ring_buffer.head - ring_buffer.tail != N {
            let mut buffer = ring_buffer.buffer;
            buffer[Self::mask(&ring_buffer.head)] = value;
            ring_buffer.head = ring_buffer.head.wrapping_add(1);
        }

        Ok(())
    }

    pub fn read(&self) -> Option<T> {
        let mut ring_buffer = self.0.lock().unwrap();

        if ring_buffer.tail != ring_buffer.head {
            let buffer = ring_buffer.buffer;
            let value = buffer[Self::mask(&ring_buffer.tail)];
            ring_buffer.tail = ring_buffer.tail.wrapping_add(1);
            return Some(value);
        }

        None
    }

    fn mask(index: &usize) -> usize {
        *index & (N - 1)
    }
}
