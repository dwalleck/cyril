use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use tokio::net::{UnixListener, UnixStream};

use super::{IpcError, MemoryEndpoint};
use crate::wire::BoxedStream;

pub(super) struct Listener {
    inner: UnixListener,
    path: PathBuf,
}

impl Listener {
    pub(super) async fn accept(&self) -> Result<BoxedStream, IpcError> {
        let (stream, _) = self
            .inner
            .accept()
            .await
            .map_err(|source| IpcError::Accept { source })?;
        Ok(Box::new(stream))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(error = %error, "could not remove memory runtime socket"),
        }
    }
}

pub(super) async fn bind(endpoint: &MemoryEndpoint) -> Result<Listener, IpcError> {
    let path = endpoint.as_path();
    let parent = path.parent().ok_or(IpcError::InvalidEndpoint {
        reason: "Unix runtime endpoint has no parent",
    })?;
    fs::create_dir_all(parent).map_err(|source| IpcError::CreateDirectory { source })?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|source| IpcError::Protect { source })?;
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(IpcError::Bind {
                source: io::Error::new(io::ErrorKind::AlreadyExists, "runtime endpoint exists"),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(IpcError::Bind { source }),
    }
    let inner = UnixListener::bind(path).map_err(|source| IpcError::Bind { source })?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|source| IpcError::Protect { source })?;
    Ok(Listener {
        inner,
        path: path.to_path_buf(),
    })
}

pub(super) async fn connect(endpoint: &MemoryEndpoint) -> Result<BoxedStream, IpcError> {
    let stream = UnixStream::connect(endpoint.as_path())
        .await
        .map_err(|source| IpcError::Connect { source })?;
    Ok(Box::new(stream))
}
