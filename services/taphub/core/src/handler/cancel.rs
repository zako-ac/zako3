use std::future::Future;
use std::time::{Duration, Instant};

use tokio::sync::watch;

/// How a call to a tap ended.
#[derive(Debug)]
pub(crate) enum Waited<T> {
    /// The tap answered inside its budget.
    Done(T),
    /// The budget ran out first, and the call was dropped.
    TimedOut,
    /// The tap connection went away first, and the call was dropped.
    Disconnected,
}

/// Waits on a tap call under two limits that both have to hold: the deadline the
/// whole request was given (`TapHub::request_timeout`, measured from the start of
/// the handler) and the life of the tap connection the call is being made on.
///
/// A tap request has no business outliving either one. Past the deadline the
/// caller has given up already, so whatever the tap sends afterwards has nowhere
/// to go; when the connection dies the request will never be answered at all. In
/// both cases the future is *dropped* rather than awaited to the end: the request
/// chan goes with it, and nothing on this path touches the tap connection itself,
/// which is a clone the hub owns.
///
/// Note what dropping does not do: protofish3 has no chan-close, so the tap's own
/// stream is left open until the connection ends. That is no worse than what the
/// old timeout did, and it is the reason cancellation is scoped to the request.
pub(crate) async fn wait_for_tap<T>(
    fut: impl Future<Output = T>,
    deadline: Instant,
    disconnect_rx: &mut watch::Receiver<bool>,
) -> Waited<T> {
    // A connection that is already gone is not worth a request: `changed()`
    // would wait for a next value on a watch channel that has sent its last.
    if *disconnect_rx.borrow() {
        return Waited::Disconnected;
    }

    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Waited::TimedOut;
    }

    tokio::select! {
        biased;
        _ = disconnect_rx.changed() => Waited::Disconnected,
        _ = tokio::time::sleep(remaining) => Waited::TimedOut,
        out = fut => Waited::Done(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// A tap that never answers is cut off at the budget. This is what the
    /// metadata path was missing entirely: it used to await the tap forever.
    #[tokio::test]
    async fn a_silent_tap_ends_at_the_budget() {
        let (_tx, mut rx) = watch::channel(false);

        let start = Instant::now();
        let out = wait_for_tap(
            tokio::time::sleep(Duration::from_secs(60)),
            start + Duration::from_millis(100),
            &mut rx,
        )
        .await;

        assert!(matches!(out, Waited::TimedOut), "got {:?}", out);
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(100),
            "returned before the budget elapsed: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "waited for the tap instead of the budget: {elapsed:?}"
        );
    }

    /// A tap disconnect cuts the wait off immediately, well inside the budget:
    /// the request future must not sit out the timeout it was given.
    #[tokio::test]
    async fn a_tap_disconnect_ends_the_wait_before_the_budget() {
        let (tx, mut rx) = watch::channel(false);

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let _ = tx.send(true);
        });

        let start = Instant::now();
        let out = wait_for_tap(
            tokio::time::sleep(Duration::from_secs(60)),
            start + Duration::from_secs(5),
            &mut rx,
        )
        .await;

        assert!(matches!(out, Waited::Disconnected), "got {:?}", out);
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(1),
            "disconnect was not immediate: {elapsed:?}"
        );
    }

    /// A connection already known to be gone costs no request at all.
    #[tokio::test]
    async fn an_already_dead_tap_is_not_called() {
        let (tx, mut rx) = watch::channel(false);
        tx.send(true).expect("send");

        let start = Instant::now();
        let out = wait_for_tap(
            tokio::time::sleep(Duration::from_secs(60)),
            start + Duration::from_secs(5),
            &mut rx,
        )
        .await;

        assert!(matches!(out, Waited::Disconnected), "got {:?}", out);
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "polled a dead tap: {:?}",
            start.elapsed()
        );
    }

    /// An answer inside the budget is passed through untouched.
    #[tokio::test]
    async fn an_answer_inside_the_budget_is_returned() {
        let (_tx, mut rx) = watch::channel(false);

        let out = wait_for_tap(
            async { 7u8 },
            Instant::now() + Duration::from_secs(5),
            &mut rx,
        )
        .await;

        assert!(matches!(out, Waited::Done(7)), "got {:?}", out);
    }

    /// A request whose budget is already spent does not start a tap call at all.
    #[tokio::test]
    async fn a_spent_budget_does_not_call_the_tap() {
        let (_tx, mut rx) = watch::channel(false);

        let start = Instant::now();
        let out = wait_for_tap(
            tokio::time::sleep(Duration::from_secs(60)),
            start - Duration::from_millis(1),
            &mut rx,
        )
        .await;

        assert!(matches!(out, Waited::TimedOut), "got {:?}", out);
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "called a tap with no budget left: {:?}",
            start.elapsed()
        );
    }
}
