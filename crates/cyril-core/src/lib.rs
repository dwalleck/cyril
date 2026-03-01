pub mod protocol;
pub mod platform;
pub mod session;
pub mod event;
pub mod kiro_ext;
pub mod capabilities;
pub mod hooks;

// Re-exports for backwards compatibility
pub use protocol::client;
pub use protocol::transport;
pub use platform::path;
