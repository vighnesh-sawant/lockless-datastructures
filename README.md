
# lockless-datastructures
I implemented an mpmc and spsc atomic ring buffer to learn about the memory models of modern cpu's
As per my naive benchmarks the atomic ring buffer's turned out to be faster than the mutex based ring buffer

On the way even managed to write code which deadlocked(in the mutex ring buffer) and learned how mistakes are made!  
This ring buffer's are used in high performance packet processing, the network directly writes into a ring buffer (dma)
and the cpu picks them up when its free to.  
I actually SAW the difference cache collisions make in performance (quite a lot btw)  

# Mutex Ring Buffer
Simple ring buffer with a mutex over the whole list (imho the source of perfomance difference)

# Atomic Ring Buffer Spsc
Head and tail are declared as atomics, while reading and writing to them, I make sure appropriate memory fences are used.

# Atomic Ring Buffer Mpmc
In this I maintain a sequence number is each slot, the writer checks if the sequence number is equal to head then only it starts writing.
On finishing writing it increments the sequence number by 1. The sequence number are init to be equal to the index so,the reader
looks if the sequence is equal to tail+1, then only does it read the slot.The reader on finishing reading makes the sequence number
tail + N to make it ready for the next writer to write!
All this additions are wrapped to handle even big numbers!
The head and tail are modulo N ( I take N as a power of 2, so that I can do modulo with bit masking)

# Use this?
You can test this out by cloning the repo( not from crates.io it does not have the benches code) and running cargo bench!  
This will give you the stats comparing mutex and lockless datastructures!  
Go to target/criterion/report/index.html to look at the COOL graphs  
The bench is just a mock multithreaded application that stress tests my library.  
I also have a website demonstrating ring-buffers which uses my library [here](https://vighnesh-sawant.github.io/lockless-datastructures/)  
This is supposed to be a library that I make use of in my upcoming projects!  
Add [this](https://crates.io/crates/lockless-datastructures) to your project using cargo and use AtomicRingBufferSpsc::new() to create a new ring buffer!  
Use the push and pop functions!  
[Docs](https://docs.rs/lockless-datastructures/0.1.0/lockless_datastructures/index.html)  
Boom now you have a ring buffer, with push and pop,all your high performance networking dreams are 
fulfilled!!  
The demo just demoing that my ring buffer works!

