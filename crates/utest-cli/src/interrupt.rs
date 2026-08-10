//! Graceful racing of suite execution against Ctrl+C.

use std::{future::Future, io};

/// Outcome of an operation raced against the process interrupt signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterruptOutcome<T> {
    /// The operation completed before interruption.
    Completed(T),
    /// Ctrl+C was observed and the operation future was cancelled by dropping it.
    Interrupted,
}

/// Runs `operation` until completion or the first Ctrl+C signal.
///
/// # Errors
///
/// Returns an I/O error when Tokio cannot install or monitor the platform
/// interrupt handler.
pub(crate) async fn run_until_ctrl_c<F>(operation: F) -> io::Result<InterruptOutcome<F::Output>>
where
    F: Future,
{
    race(operation, tokio::signal::ctrl_c()).await
}

async fn race<F, S>(operation: F, interrupt: S) -> io::Result<InterruptOutcome<F::Output>>
where
    F: Future,
    S: Future<Output = io::Result<()>>,
{
    tokio::select! {
        biased;
        signal = interrupt => signal.map(|()| InterruptOutcome::Interrupted),
        output = operation => Ok(InterruptOutcome::Completed(output)),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::{self, Future},
        io,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll},
    };

    use super::{InterruptOutcome, race};

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    struct ReadyWithDrop {
        dropped: Arc<AtomicBool>,
    }

    impl Future for ReadyWithDrop {
        type Output = u8;

        fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Ready(37)
        }
    }

    impl Drop for ReadyWithDrop {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn returns_completed_output_when_operation_wins() {
        let result = runtime()
            .block_on(race(
                future::ready(37_u8),
                future::pending::<io::Result<()>>(),
            ))
            .expect("signal monitor");
        assert_eq!(result, InterruptOutcome::Completed(37));
    }

    #[test]
    fn interruption_has_priority_and_drops_a_simultaneously_ready_operation() {
        let dropped = Arc::new(AtomicBool::new(false));
        let result = runtime()
            .block_on(race(
                ReadyWithDrop {
                    dropped: Arc::clone(&dropped),
                },
                future::ready(Ok(())),
            ))
            .expect("signal monitor");
        assert_eq!(result, InterruptOutcome::Interrupted);
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn preserves_signal_monitor_errors() {
        let result = runtime().block_on(race(
            future::pending::<u8>(),
            future::ready(Err(io::Error::other("signal unavailable"))),
        ));
        assert_eq!(
            result.expect_err("signal error").kind(),
            io::ErrorKind::Other
        );
    }
}
