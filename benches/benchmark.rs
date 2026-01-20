use criterion::{Criterion, criterion_group, criterion_main};
use lockless_datastructures::MutexRingBuffer;
use std::hint::black_box;

const CAPACITY: usize = 1024;
const OPERATIONS: usize = 1000;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("MutexRingBuffer", |b| b.iter(|| ring_buffer_benchmark()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

fn ring_buffer_benchmark() {
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
