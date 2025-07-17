use std::{
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

struct CounterFuture {
    waker: Arc<Mutex<Option<Waker>>>,
    waker_ref: Arc<AtomicU32>,
    poll_count: Arc<AtomicU32>,
}

const ITERATIONS: u32 = 10;

fn do_work(
    thread_waker: &Arc<Mutex<Option<Waker>>>,
    waker_ref: &Arc<AtomicU32>,
    poll_count: &Arc<AtomicU32>,
) {
    // Each thread sleeps for the same duration, so that we intentionally introduce a race condition
    // and ensure that multiple threads try to wake up the task
    thread::sleep(Duration::from_millis(100));

    // Atomically increase the counter without using a Mutex<T>
    poll_count.fetch_add(1, Ordering::SeqCst);

    // Not each thread will get Some because they are competing for the waker
    if let Some(waker) = thread_waker.lock().unwrap().take() {
        waker_ref.fetch_add(1, Ordering::SeqCst);
        println!(
            "Waker: {waker:?}, ref: {}, poll_count: {}",
            waker_ref.load(Ordering::SeqCst),
            poll_count.load(Ordering::SeqCst),
        );
        waker.wake();
    }
}

impl CounterFuture {
    fn new() -> Self {
        let waker: Arc<Mutex<Option<Waker>>> = Arc::default();
        let waker_ref = Arc::new(AtomicU32::new(0));
        let poll_count = Arc::new(AtomicU32::new(0));

        // Start a thread that sleeps, then increases poll_count, then tries to wake up the task
        for _ in 0..ITERATIONS {
            let thread_waker = waker.clone();
            let thread_waker_ref = waker_ref.clone();
            let poll_count = poll_count.clone();

            thread::spawn(move || do_work(&thread_waker, &thread_waker_ref, &poll_count));
        }
        Self {
            waker,
            waker_ref,
            poll_count,
        }
    }
}

impl Future for CounterFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut waker = self.waker.lock().unwrap();
        let count = self.poll_count.load(Ordering::Relaxed);
        let waker_ref = self.waker_ref.load(Ordering::SeqCst);
        println!("Poll count from poll(): {count}, waker_ref: {waker_ref}");
        if count == ITERATIONS {
            Poll::Ready(())
        } else {
            *waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    for _ in 0..10 {
        CounterFuture::new().await;
    }
    println!("Done awaiting futures.");
}
