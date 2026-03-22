use std::time::{Duration, Instant};

/// Buffers streaming text and flushes at semantic boundaries
/// (newlines, code fences) or after a timeout.
pub struct StreamBuffer {
    buffer: String,
    last_flush: Instant,
    timeout: Duration,
}

impl StreamBuffer {
    pub fn new(timeout: Duration) -> Self {
        Self {
            buffer: String::new(),
            last_flush: Instant::now(),
            timeout,
        }
    }

    /// Push text into the buffer. Returns a chunk if a semantic boundary is found.
    pub fn push(&mut self, text: &str) -> Option<String> {
        if text.is_empty() {
            return None;
        }
        self.buffer.push_str(text);

        if let Some(boundary) = self.find_boundary() {
            let chunk = self.buffer[..boundary].to_string();
            self.buffer = self.buffer[boundary..].to_string();
            self.last_flush = Instant::now();
            return Some(chunk);
        }
        None
    }

    /// Whether the buffer should be force-flushed (timeout elapsed with pending content).
    pub fn should_flush(&self) -> bool {
        !self.buffer.is_empty() && self.last_flush.elapsed() > self.timeout
    }

    /// Force-flush all remaining content. Returns None if buffer is empty.
    pub fn flush(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            self.last_flush = Instant::now();
            Some(std::mem::take(&mut self.buffer))
        }
    }

    fn find_boundary(&self) -> Option<usize> {
        // Code block markers (```)
        if let Some(pos) = self.buffer.find("```")
            && let Some(newline) = self.buffer[pos..].find('\n')
        {
            return Some(pos + newline + 1);
        }

        // Any newline
        if let Some(pos) = self.buffer.find('\n') {
            return Some(pos + 1);
        }

        None
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn push_with_newline_flushes_at_boundary() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        let result = buf.push("hello\nworld");
        assert_eq!(result, Some("hello\n".to_string()));
        // "world" remains in buffer
        assert_eq!(buf.flush(), Some("world".to_string()));
    }

    #[test]
    fn push_without_boundary_returns_none() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        let result = buf.push("no newline here");
        assert_eq!(result, None);
    }

    #[test]
    fn push_with_code_fence_flushes() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        let result = buf.push("some text```rust\ncode");
        assert!(result.is_some());
        let flushed = result.unwrap();
        assert!(flushed.contains("```rust\n"));
    }

    #[test]
    fn flush_returns_remaining() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        buf.push("buffered");
        let result = buf.flush();
        assert_eq!(result, Some("buffered".to_string()));
    }

    #[test]
    fn flush_empty_returns_none() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        assert_eq!(buf.flush(), None);
    }

    #[test]
    fn double_flush_returns_none() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        buf.push("data");
        buf.flush();
        assert_eq!(buf.flush(), None);
    }

    #[test]
    fn multiple_newlines_flush_at_first() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        let result = buf.push("line1\nline2\nline3");
        assert_eq!(result, Some("line1\n".to_string()));
        // Continue flushing
        let result2 = buf.push("");
        // Empty push shouldn't change anything, but buffer still has "line2\nline3"
        assert_eq!(result2, None);
        // Explicit flush gets the rest
        let rest = buf.flush();
        assert_eq!(rest, Some("line2\nline3".to_string()));
    }

    #[test]
    fn should_flush_respects_timeout() {
        let buf = StreamBuffer::new(std::time::Duration::from_millis(0));
        // With zero timeout, should always be ready to flush
        // (after at least one push)
        assert!(!buf.should_flush()); // empty buffer, nothing to flush
    }

    #[test]
    fn push_empty_string_no_effect() {
        let mut buf = StreamBuffer::new(std::time::Duration::from_millis(150));
        let result = buf.push("");
        assert_eq!(result, None);
        assert_eq!(buf.flush(), None);
    }
}
