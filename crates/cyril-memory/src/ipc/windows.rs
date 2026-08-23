use std::io;

use interprocess::os::windows::named_pipe::tokio::{PipeListener, PipeStream};
use interprocess::os::windows::named_pipe::{PipeListenerOptions, pipe_mode};
use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use widestring::U16CString;
use win_security_identifier::{GetCurrentSid, SecurityIdentifier};

use super::{IpcError, MemoryEndpoint};
use crate::wire::BoxedStream;

pub(super) struct Listener {
    inner: PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>,
}

impl Listener {
    pub(super) async fn accept(&self) -> Result<BoxedStream, IpcError> {
        let stream = self
            .inner
            .accept()
            .await
            .map_err(|source| IpcError::Accept { source })?;
        Ok(Box::new(stream))
    }
}

pub(super) async fn bind(endpoint: &MemoryEndpoint) -> Result<Listener, IpcError> {
    let current_user =
        SecurityIdentifier::get_current_user_sid().map_err(|error| IpcError::Protect {
            source: io::Error::new(io::ErrorKind::PermissionDenied, error),
        })?;
    let sddl = U16CString::from_str(format!("D:P(A;;GA;;;{current_user})")).map_err(|error| {
        IpcError::Protect {
            source: io::Error::new(io::ErrorKind::InvalidData, error),
        }
    })?;
    let descriptor = SecurityDescriptor::deserialize(sddl.as_ucstr())
        .map_err(|source| IpcError::Protect { source })?;
    let inner = PipeListenerOptions::new()
        .path(endpoint.as_path())
        .accept_remote(false)
        .security_descriptor(Some(descriptor))
        .create_tokio_duplex::<pipe_mode::Bytes>()
        .map_err(|source| IpcError::Bind { source })?;
    Ok(Listener { inner })
}

pub(super) async fn connect(endpoint: &MemoryEndpoint) -> Result<BoxedStream, IpcError> {
    let stream =
        PipeStream::<pipe_mode::Bytes, pipe_mode::Bytes>::connect_by_path(endpoint.as_path())
            .await
            .map_err(|source| IpcError::Connect { source })?;
    Ok(Box::new(stream))
}
