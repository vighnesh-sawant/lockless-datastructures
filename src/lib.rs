use std::{
    cell::UnsafeCell,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
struct RingBuffer<T, const N: usize> {
    buffer: UnsafeCell<[T; N]>,
    head: Mutex<usize>,
    tail: Mutex<usize>,
}

#[derive(Debug, Clone)]
pub struct MutexRingBuffer<T, const N: usize>(Arc<RingBuffer<T, N>>);

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
        Self(Arc::new(RingBuffer {
            buffer: UnsafeCell::new(std::array::from_fn(|_| T::default())),
            head: Mutex::new(0),
            tail: Mutex::new(0),
        }))
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let mut head = match self.0.head.lock() {
            Ok(lock) => lock,
            Err(_) => return Err(value),
        };

        if *head < N {
            let buffer = self.0.buffer.get();
            unsafe {
                (*buffer)[Self::mask(&head)] = value;
            }
            *head = head.wrapping_add(1);
        } else {
            println!("Dropping data as buffer is full");
        }

        Ok(())
    }

    pub fn read(&self) -> Option<T> {
        let mut tail = self.0.tail.lock().unwrap();

        let head = {
            let guard = self.0.head.lock().unwrap(); // Read lock acquired
            *guard // Read (clone for immediate release)
        };
        if *tail != head {
            let buffer = self.0.buffer.get();
            let value;
            unsafe {
                value = (*buffer)[Self::mask(&tail)];
            }
            *tail = tail.wrapping_add(1);
            return Some(value);
        }

        None
    }

    // pub fn size(&self) -> usize {
    //     let head = self.0.head.lock().unwrap();
    //     let tail = self.0.tail.lock().unwrap();
    //
    //     *head - *tail
    // }
    // pub fn empty(&self) -> bool {
    //     let head = self.0.head.lock().unwrap();
    //     let tail = self.0.tail.lock().unwrap();
    //
    //     *head == *tail
    // }

    // pub fn try_read(&self) -> Result<T, ()> {
    // let mut buffer = self.buffer.lock().map_err(|_| ())?;
    // buffer.pop_front().ok_or(())
    // }

    fn mask(index: &usize) -> usize {
        *index & (N - 1)
    }
}
