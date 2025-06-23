//! # cancellable_loops
//!
//! A small utility crate for creating cancellable loops in both sequential and parallel contexts.
//!
//! This crate provides functions that allow you to break out of loops early when an abort flag is set,
//! which is particularly useful for:
//!
//! - Long-running computations that need cancellation support
//! - User-interruptible processing pipelines
//! - Tasks that may need to be aborted based on external conditions
//!
//! ## Features
//!
//! - Sequential loop with cancellation support
//! - Parallel loop with cancellation support using Rayon
//! - Parallel loop with both cancellation and reduction
//!
//! ## Example
//!
//! ```
//! use std::sync::atomic::{AtomicBool, Ordering};
//! use std::thread;
//! use std::time::Duration;
//! use cancellable_loops::for_each_cancellable;
//!
//! let abort_flag = AtomicBool::new(false);
//! let abort_handle = &abort_flag;
//!
//! // In another thread, set the abort flag after some time
//! let handle = thread::spawn(move || {
//!     thread::sleep(Duration::from_millis(10));
//!     abort_flag.store(true, Ordering::Relaxed);
//! });
//!
//! // This loop will exit early when the abort flag is set
//! for_each_cancellable(0..1000, abort_handle, |i| {
//!     println!("Processing {}", i);
//!     thread::sleep(Duration::from_millis(1));
//! });
//!
//! handle.join().unwrap();
//! ```

use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Executes a sequential loop that can be cancelled via an abort flag.
///
/// This function iterates over the provided iterator and applies the given function
/// to each element. If the abort flag is set to `true` at any point during iteration,
/// the loop will exit early.
///
/// # Arguments
///
/// * `iter` - Any iterator to process
/// * `abort_flag` - An atomic boolean that can be set to `true` to cancel the loop
/// * `func` - A function to apply to each element in the iterator
///
/// # Examples
///
/// ```
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use cancellable_loops::for_each_cancellable;
///
/// let abort_flag = AtomicBool::new(false);
/// let mut sum = 0;
///
/// // This will process all items since abort_flag is never set
/// for_each_cancellable(1..=10, &abort_flag, |i| {
///     sum += i;
///     
///     // Simulate a condition to abort after reaching 15
///     if sum > 15 {
///         abort_flag.store(true, Ordering::Relaxed);
///     }
/// });
///
/// // The loop was cancelled after processing 5 elements (1+2+3+4+5=15)
/// assert_eq!(sum, 15);
/// ```
pub fn for_each_cancellable<I, F, T>(iter: I, abort_flag: &AtomicBool, mut func: F)
where
    I: IntoIterator<Item = T>,
    F: FnMut(T),
{
    for item in iter {
        if abort_flag.load(Ordering::Relaxed) {
            break;
        }
        func(item);
    }
}

/// Executes a parallel loop that can be cancelled via an abort flag.
///
/// This function parallelizes the iteration over the provided iterator using Rayon
/// and applies the given function to each element. If the abort flag is set to `true`
/// at any point, remaining work will be skipped.
///
/// # Arguments
///
/// * `iter` - Any parallel iterator to process
/// * `abort_flag` - An atomic boolean that can be set to `true` to cancel the loop
/// * `func` - A function to apply to each element in the iterator
///
/// # Examples
///
/// ```
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::{Arc, Mutex};
/// use cancellable_loops::par_for_each_cancellable;
///
/// let abort_flag = AtomicBool::new(false);
/// let counter = Arc::new(Mutex::new(0));
/// let counter_clone = counter.clone();
///
/// // Process items in parallel
/// par_for_each_cancellable(1..100, &abort_flag, move |i| {
///     // Simulate some work
///     std::thread::sleep(std::time::Duration::from_millis(1));
///     
///     let mut count = counter_clone.lock().unwrap();
///     *count += 1;
///     
///     // Cancel after processing at least 10 items
///     if *count >= 10 {
///         abort_flag.store(true, Ordering::Relaxed);
///     }
/// });
///
/// // We processed at least 10 items before cancellation
/// assert!(*counter.lock().unwrap() >= 10);
/// ```
pub fn par_for_each_cancellable<I, F>(iter: I, abort_flag: &AtomicBool, func: F)
where
    I: IntoParallelIterator,
    F: Fn(I::Item) + Sync + Send,
    I::Item: Send,
{
    let abort = Arc::new(abort_flag);

    iter.into_par_iter()
        .try_for_each(|item| {
            if abort.load(Ordering::Relaxed) {
                Err(())
            } else {
                func(item);
                Ok(())
            }
        })
        .ok();
}

/// Executes a parallel loop with reduction that can be cancelled via an abort flag.
///
/// This function parallelizes the iteration over the provided iterator using Rayon,
/// applies the given function to each element, and then reduces the results using
/// the provided reducer function. If the abort flag is set to `true` at any point,
/// remaining work will be skipped, and only the reduction of already processed items
/// will be returned.
///
/// # Arguments
///
/// * `iter` - Any parallel iterator to process
/// * `abort_flag` - An atomic boolean that can be set to `true` to cancel the loop
/// * `func` - A function that takes an item and returns an optional result
/// * `reducer` - A function that combines two results
/// * `init` - The initial value for the reduction
///
/// # Returns
///
/// The reduced result of all processed items, or the initial value if no items were processed.
///
/// # Examples
///
/// ```
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use cancellable_loops::par_for_each_cancellable_reduce;
///
/// let abort_flag = AtomicBool::new(false);
///
/// // Sum all numbers until we reach a threshold
/// let sum = par_for_each_cancellable_reduce(
///     1..1000,
///     &abort_flag,
///     |i| {
///         // Simulate some work
///         std::thread::sleep(std::time::Duration::from_millis(1));
///         
///         // Cancel if we see a large number
///         if i > 50 {
///             abort_flag.store(true, Ordering::Relaxed);
///         }
///         
///         Some(i)
///     },
///     |a, b| a + b,
///     0
/// );
///
/// // We processed some items before hitting the threshold
/// assert!(sum > 0);
/// ```
pub fn par_for_each_cancellable_reduce<I, F, R>(
    iter: I,
    abort_flag: &AtomicBool,
    func: F,
    reducer: impl Fn(R, R) -> R + Sync + Send,
    init: R,
) -> R
where
    I: IntoParallelIterator,
    F: Fn(I::Item) -> Option<R> + Sync + Send,
    I::Item: Send,
    R: Send + Sync + Clone + 'static,
{
    let abort = Arc::new(abort_flag);
    iter.into_par_iter()
        .filter_map(|item| {
            if abort.load(Ordering::Relaxed) {
                None
            } else {
                func(item)
            }
        })
        .reduce(|| init.clone(), reducer)
}
