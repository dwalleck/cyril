use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;

use thiserror::Error;

use crate::wire::BoxedStream;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// A private local endpoint for the memory runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEndpoint {
    value: OsString,
}

impl MemoryEndpoint {
    /// Build a platform endpoint from an absolute runtime-directory path.
    pub fn from_path(path: &Path) -> Result<Self, IpcError> {
        if !path.is_absolute() {
            return Err(IpcError::InvalidEndpoint {
                reason: "runtime endpoint must be absolute",
            });
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            if path.as_os_str().as_bytes().len() > 100 {
                return Err(IpcError::InvalidEndpoint {
                    reason: "Unix runtime endpoint is too long",
                });
            }
            Ok(Self {
                value: path.as_os_str().to_owned(),
            })
        }
        #[cfg(windows)]
        {
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).map_err(IpcError::Random)?;
            Ok(Self {
                value: OsString::from(format!(r"\\.\pipe\cyril-memory-{}", hex::encode(random))),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Err(IpcError::InvalidEndpoint {
                reason: "local memory IPC is unsupported on this platform",
            })
        }
    }

    pub(crate) fn from_child_env(value: OsString) -> Result<Self, IpcError> {
        let endpoint = Self { value };
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            if !endpoint.as_path().is_absolute() || endpoint.to_child_env().as_bytes().len() > 100 {
                return Err(IpcError::InvalidEndpoint {
                    reason: "Unix runtime endpoint environment value is invalid",
                });
            }
        }
        #[cfg(windows)]
        {
            let valid = endpoint
                .to_child_env()
                .to_str()
                .is_some_and(|value| value.starts_with(r"\\.\pipe\cyril-memory-"));
            if !valid {
                return Err(IpcError::InvalidEndpoint {
                    reason: "Windows runtime endpoint environment value is invalid",
                });
            }
        }
        Ok(endpoint)
    }

    /// Encode the endpoint for the child environment.
    pub fn to_child_env(&self) -> &OsStr {
        &self.value
    }

    /// Return safe endpoint display metadata.
    pub fn display(&self) -> String {
        #[cfg(windows)]
        {
            "private Windows named pipe".to_owned()
        }
        #[cfg(not(windows))]
        {
            "private Unix socket".to_owned()
        }
    }

    pub(crate) fn as_path(&self) -> &Path {
        Path::new(&self.value)
    }
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("invalid memory runtime endpoint: {reason}")]
    InvalidEndpoint { reason: &'static str },
    #[error("could not create private runtime directory: {source}")]
    CreateDirectory {
        #[source]
        source: io::Error,
    },
    #[error("could not protect private runtime endpoint: {source}")]
    Protect {
        #[source]
        source: io::Error,
    },
    #[error("could not bind private runtime endpoint: {source}")]
    Bind {
        #[source]
        source: io::Error,
    },
    #[error("could not connect to private runtime endpoint: {source}")]
    Connect {
        #[source]
        source: io::Error,
    },
    #[error("could not accept private runtime connection: {source}")]
    Accept {
        #[source]
        source: io::Error,
    },
    #[error("could not generate private runtime endpoint: {0}")]
    Random(getrandom::Error),
}

pub(crate) struct IpcListener {
    #[cfg(unix)]
    inner: unix::Listener,
    #[cfg(windows)]
    inner: windows::Listener,
}

impl IpcListener {
    pub(crate) async fn accept(&self) -> Result<BoxedStream, IpcError> {
        self.inner.accept().await
    }
}

pub(crate) async fn bind(endpoint: &MemoryEndpoint) -> Result<IpcListener, IpcError> {
    #[cfg(unix)]
    let inner = unix::bind(endpoint).await?;
    #[cfg(windows)]
    let inner = windows::bind(endpoint).await?;
    #[cfg(not(any(unix, windows)))]
    return Err(IpcError::InvalidEndpoint {
        reason: "local memory IPC is unsupported on this platform",
    });
    Ok(IpcListener { inner })
}

pub(crate) async fn connect(endpoint: &MemoryEndpoint) -> Result<BoxedStream, IpcError> {
    #[cfg(unix)]
    return unix::connect(endpoint).await;
    #[cfg(windows)]
    return windows::connect(endpoint).await;
    #[cfg(not(any(unix, windows)))]
    Err(IpcError::InvalidEndpoint {
        reason: "local memory IPC is unsupported on this platform",
    })
}
