use criterion::{Criterion, criterion_group, criterion_main};
use lockless_datastructures::{
    atomic_ring_buffer_mpmc::AtomicRingBufferMpmc, atomic_ring_buffer_spsc::AtomicRingBufferSpsc,
    mutex_ring_buffer::MutexRingBuffer,
};
use std::{
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const CAPACITY: usize = 1024;
const OPERATIONS: usize = 1000;

#[allow(clippy::redundant_closure)]
fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("SPSC");
    group.bench_function("MutexRingBufferSpsc", |b| {
        b.iter(|| mutex_ring_buffer_spsc_benchmark())
    });
    group.bench_function("AtomicRingBufferSpsc", |b| {
        b.iter(|| atomic_ring_buffer_spsc_benchmark())
    });
    group.finish();
    let mut group = c.benchmark_group("MPMC");
    group.bench_function("MutexRingBufferMpmc", |b| {
        b.iter(|| mutex_ring_buffer_mpmc_benchmark())
    });
    group.bench_function("AtomicRingBufferMpmc", |b| {
        b.iter(|| atomic_ring_buffer_mpmc_benchmark())
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

fn mutex_ring_buffer_spsc_benchmark() {
    let buffer: MutexRingBuffer<i32, CAPACITY> = MutexRingBuffer::new();

    let producer_buffer = buffer.clone();
    let consumer_buffer = buffer.clone();

    let producer = std::thread::spawn(move || {
        for i in 0..OPERATIONS {
            producer_buffer.push(black_box(i as i32)).unwrap();
        }
    });

    let consumer = std::thread::spawn(move || {
        let mut count = 0;
        while count < OPERATIONS {
            if let Some(value) = consumer_buffer.read() {
                black_box(value);
                count += 1;
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

fn atomic_ring_buffer_spsc_benchmark() {
    let buffer = AtomicRingBufferSpsc::<i32, CAPACITY>::new();

    let producer_buffer = buffer.clone();
    let consumer_buffer = buffer.clone();

    let producer = std::thread::spawn(move || {
        for i in 0..OPERATIONS {
            producer_buffer.push(black_box(i as i32)).unwrap();
        }
    });

    let consumer = std::thread::spawn(move || {
        let mut count = 0;
        while count < OPERATIONS {
            if let Some(value) = consumer_buffer.read() {
                black_box(value);
                count += 1;
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

fn mutex_ring_buffer_mpmc_benchmark() {
    let buffer: MutexRingBuffer<i32, CAPACITY> = MutexRingBuffer::new();

    let consumed_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(4);

    let ops_per_producer = OPERATIONS / 2;

    for _ in 0..2 {
        let buf = buffer.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..ops_per_producer {
                while buf.push(black_box(i as i32)).is_err() {
                    std::hint::spin_loop();
                }
            }
        }));
    }

    for _ in 0..2 {
        let buf = buffer.clone();
        let counter = consumed_count.clone();
        handles.push(std::thread::spawn(move || {
            loop {
                if counter.load(Ordering::Relaxed) >= OPERATIONS {
                    break;
                }

                if let Some(val) = buf.read() {
                    black_box(val);
                    counter.fetch_add(1, Ordering::Relaxed);
                } else {
                    std::hint::spin_loop();
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn atomic_ring_buffer_mpmc_benchmark() {
    let buffer = AtomicRingBufferMpmc::<i32, CAPACITY>::new();

    let consumed_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(4);

    let ops_per_producer = OPERATIONS / 2;

    for _ in 0..2 {
        let buf = buffer.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..ops_per_producer {
                while buf.push(black_box(i as i32)).is_err() {
                    std::hint::spin_loop();
                }
            }
        }));
    }

    for _ in 0..2 {
        let buf = buffer.clone();
        let counter = consumed_count.clone();
        handles.push(std::thread::spawn(move || {
            loop {
                if counter.load(Ordering::Relaxed) >= OPERATIONS {
                    break;
                }

                if let Some(val) = buf.read() {
                    black_box(val);
                    counter.fetch_add(1, Ordering::Relaxed);
                } else {
                    std::hint::spin_loop();
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}
