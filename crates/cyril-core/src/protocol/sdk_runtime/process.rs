use std::pin::Pin;
#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use agent_client_protocol::{Client, ConnectTo, Lines, Result as AcpResult};
use futures_util::{AsyncBufReadExt as _, AsyncWriteExt as _, Stream};
#[cfg(test)]
use tokio::io::{AsyncRead, ReadBuf};
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};

use crate::protocol::transport::AgentProcess;

/// The official SDK transport component for Cyril's retained process owner.
///
/// `AgentProcess` remains responsible for spawning, stderr collection, cwd,
/// and process-tree cleanup. This adapter only converts its two pipes to the
/// SDK's `Lines` role. No JSON parsing or frame rewriting occurs before the
/// official SDK transport owns the stream.
pub(crate) struct ProcessAdapter {
    process: AgentProcess,
    eof_line: String,
    #[cfg(test)]
    capture: Option<Arc<Mutex<Vec<u8>>>>,
}

impl ProcessAdapter {
    pub(crate) fn new(process: AgentProcess, eof_line: String) -> Self {
        Self {
            process,
            eof_line,
            #[cfg(test)]
            capture: None,
        }
    }

    #[cfg(test)]
    pub(super) fn new_recording(
        process: AgentProcess,
        eof_line: String,
        capture: Arc<Mutex<Vec<u8>>>,
    ) -> Self {
        Self {
            process,
            eof_line,
            capture: Some(capture),
        }
    }
}

/// Appends one private `_cyril.internal/transport_closed` frame when the
/// agent's stdout reaches EOF, converting "the transport died" into an
/// ordered IN-BAND event.
///
/// Deliberate design (PR #115 review, finding 12): SDK 2.0 offers
/// `Builder::on_close` as the out-of-band alternative — "pending requests are
/// failed before callbacks begin" — and for a DIRECT transport its ordering
/// is documented. Cyril's client, however, sits above a
/// `ConductorImpl` stage chain (the platform's whole point), and nothing in
/// the conductor documents that a close on the agent-side transport reaches
/// the client-side connection only AFTER every already-received frame has
/// been dispatched through every stage. The in-band marker keeps that FIFO
/// property by construction: it rides the same serial line as the data
/// frames, so any stage that forwards frames in order preserves
/// "all-frames-then-close" end to end. The random token (generated per
/// connection in `DomainChannels::new`) prevents the agent from spoofing the
/// marker; proxy stages will observe the clearly-namespaced frame, which is
/// part of the contract, not leakage.
struct EofMarkerStream<S> {
    inner: Pin<Box<S>>,
    marker: Option<String>,
}

impl<S> EofMarkerStream<S> {
    fn new(inner: S, marker: String) -> Self {
        Self {
            inner: Box::pin(inner),
            marker: Some(marker),
        }
    }
}

impl<S> Stream for EofMarkerStream<S>
where
    S: Stream<Item = std::io::Result<String>>,
{
    type Item = std::io::Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        match this.inner.as_mut().poll_next(context) {
            Poll::Ready(None) => match this.marker.take() {
                Some(marker) => Poll::Ready(Some(Ok(marker))),
                None => Poll::Ready(None),
            },
            result => result,
        }
    }
}

#[cfg(test)]
pub(super) struct RecordingReader<R> {
    inner: R,
    capture: Option<Arc<Mutex<Vec<u8>>>>,
}

#[cfg(test)]
impl<R> RecordingReader<R> {
    pub(super) fn new(inner: R, capture: Option<Arc<Mutex<Vec<u8>>>>) -> Self {
        Self { inner, capture }
    }
}

#[cfg(test)]
impl<R> AsyncRead for RecordingReader<R>
where
    R: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(context, buffer);
        if matches!(result, Poll::Ready(Ok(())))
            && let Some(capture) = &self.capture
        {
            capture
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(&buffer.filled()[before..]);
        }
        result
    }
}

impl ConnectTo<Client> for ProcessAdapter {
    async fn connect_to(
        self,
        client: impl ConnectTo<agent_client_protocol::Agent>,
    ) -> AcpResult<()> {
        #[cfg(test)]
        let capture = self.capture;
        let eof_line = self.eof_line;
        let parts = self.process.into_parts();
        let crate::protocol::transport::AgentProcessParts {
            stdin,
            stdout,
            _child,
            #[cfg(unix)]
            _group_guard,
        } = parts;
        // Keep these resources alive until the SDK connection has completely
        // shut down. Dropping either early would alter lifecycle and
        // grandchild cleanup semantics.
        let _child = _child;
        #[cfg(unix)]
        let _group_guard = _group_guard;

        #[cfg(test)]
        let stdout = RecordingReader::new(stdout, capture);
        let incoming = EofMarkerStream::new(
            futures_util::io::BufReader::new(stdout.compat()).lines(),
            eof_line,
        );
        let outgoing = futures_util::sink::unfold(
            Box::pin(stdin.compat_write()),
            async move |mut writer, line: String| {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
                Ok::<_, std::io::Error>(writer)
            },
        );
        let lines = Lines::new(outgoing, Box::pin(incoming));
        <Lines<_, _> as ConnectTo<Client>>::connect_to(lines, client).await
    }
}
