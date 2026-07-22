use std::collections::VecDeque;

/// Bounded byte ring for session output. Overflow drops the oldest bytes so an
/// `attach` replay always shows the most recent `cap` bytes of the session.
pub struct Scrollback {
    bytes: VecDeque<u8>,
    cap: usize,
}

impl Scrollback {
    pub fn new(cap: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(cap.min(64 * 1024)),
            cap,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes.iter().copied());
        while self.bytes.len() > self.cap {
            self.bytes.pop_front();
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_pushed_bytes_in_order() {
        let mut sb = Scrollback::new(16);
        sb.push(b"hello ");
        sb.push(b"world");
        assert_eq!(sb.snapshot(), b"hello world");
    }

    #[test]
    fn overflow_drops_oldest_bytes() {
        let mut sb = Scrollback::new(8);
        sb.push(b"abcdefgh");
        sb.push(b"XY");
        assert_eq!(sb.snapshot(), b"cdefghXY");
        assert_eq!(sb.len(), 8);
    }

    #[test]
    fn push_larger_than_cap_keeps_tail() {
        let mut sb = Scrollback::new(4);
        sb.push(b"0123456789");
        assert_eq!(sb.snapshot(), b"6789");
    }

    #[test]
    fn empty_snapshot_is_empty() {
        let sb = Scrollback::new(4);
        assert!(sb.is_empty());
        assert_eq!(sb.snapshot(), Vec::<u8>::new());
    }
}
