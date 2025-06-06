use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[allow(dead_code)]
pub fn for_each_cancellable<I, F, T>(iter: I, abort_flag: &AtomicBool, mut func: F)
where
    I: IntoIterator<Item = T>,
    F: FnMut(T),
{
    for item in iter {
        if abort_flag.load(Ordering::Relaxed) {
            abort_flag.store(false, Ordering::Relaxed);
            break;
        }
        func(item);
    }
}

/// Cancels a parallel loop early if `abort_flag` is set.
/// The loop body returns `()` and is skipped if cancellation is requested.
#[allow(dead_code)]
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
                abort.store(false, Ordering::Relaxed);
                Err(()) // cancel
            } else {
                func(item);
                Ok(())
            }
        })
        .ok(); // suppress error — just treat as cancel
}

#[allow(dead_code)]
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
