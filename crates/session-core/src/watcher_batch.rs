use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

/// Collect events until no new event arrives for a full `quiet_period`.
///
/// File watchers often emit several events for one write. A trailing debounce
/// keeps all paths in the burst while still collapsing them into one refresh.
pub fn collect_until_quiet<T, F>(
    first: T,
    quiet_period: Duration,
    max_wait: Duration,
    mut receive: F,
) -> Vec<T>
where
    F: FnMut(Duration) -> Result<T, RecvTimeoutError>,
{
    let mut events = vec![first];
    let started = Instant::now();
    loop {
        let remaining = max_wait.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break;
        }
        match receive(quiet_period.min(remaining)) {
            Ok(event) => events.push(event),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn debounce_batch_keeps_every_queued_event() {
        let (sender, receiver) = mpsc::channel();
        sender.send("second").unwrap();
        sender.send("third").unwrap();
        drop(sender);

        let events = collect_until_quiet(
            "first",
            Duration::from_millis(1),
            Duration::from_millis(5),
            |timeout| receiver.recv_timeout(timeout),
        );

        assert_eq!(events, vec!["first", "second", "third"]);
    }
}
