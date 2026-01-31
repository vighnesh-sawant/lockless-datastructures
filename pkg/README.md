# lockless-datastructures
I implemented an mpmc and spsc atomic ring buffer to learn about the memory models of modern cpu's
As per my naive benchmarks the atomic ring buffer's turned out to be faster than the mutex based ring buffer
On the way even managed to write code which deadlocked(in the mutex ring buffer) and learned how mistakes are made!
This ring buffer's are used in high performance packet processing, the network directly writes into a ring buffer (dma)
and the cpu picks them up when its free to.
I actually SAW the difference cache collisions make in performance (quite a lot btw)
The mutex uses atomic instructions internally to provide mutual exclusivity.
