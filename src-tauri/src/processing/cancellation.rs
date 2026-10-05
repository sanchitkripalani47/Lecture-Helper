use std::sync::atomic::{AtomicBool, Ordering};

/// Cooperative cancellation shared between the coordinator, the processor, and
/// the `cancel_batch` command.
#[derive(Debug, Default)]
pub struct CancellationToken {
    flag: AtomicBool,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
        }
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_uncancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn reports_cancellation_once_requested() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }
}
