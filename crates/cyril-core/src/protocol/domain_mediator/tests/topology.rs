use agent_client_protocol::UntypedMessage;

use super::super::{DomainChannels, DomainWork, WORK_CAPACITY};
#[cfg(not(feature = "kas"))]
use super::super::{HOST_CAPACITY, HostWork};
use crate::protocol::source_observer::IngressTracker;

fn work(index: usize) -> DomainWork {
    let message = UntypedMessage::new(
        "session/update",
        serde_json::json!({"index": index, "update": {"sessionUpdate": "future"}}),
    )
    .unwrap_or_else(|error| panic!("capacity fixture is valid: {error}"));
    DomainWork::UnknownSessionUpdate(message)
}

#[test]
fn domain_work_capacity_is_exact_and_lossless_until_full() {
    let (channels, _work_rx, _host_rx) = DomainChannels::new(IngressTracker::new())
        .unwrap_or_else(|error| panic!("domain capacity channels: {error}"));
    for index in 0..WORK_CAPACITY {
        assert!(
            channels.work_tx.try_send(work(index)).is_ok(),
            "slot {index} must accept work"
        );
    }
    assert!(matches!(
        channels.work_tx.try_send(work(WORK_CAPACITY)),
        Err(tokio::sync::mpsc::error::TrySendError::Full(
            DomainWork::UnknownSessionUpdate(message)
        )) if message.params["index"] == WORK_CAPACITY
    ));
}

#[cfg(not(feature = "kas"))]
fn host_work(index: usize) -> HostWork {
    HostWork::Probe {
        index,
        _padding: [0; 288],
    }
}

#[cfg(not(feature = "kas"))]
#[test]
fn host_work_capacity_is_exact_and_fifo() {
    let (channels, _work_rx, mut host_rx) = DomainChannels::new(IngressTracker::new())
        .unwrap_or_else(|error| panic!("host capacity channels: {error}"));
    for index in 0..HOST_CAPACITY {
        assert!(
            channels.host_tx.try_send(host_work(index)).is_ok(),
            "host slot {index} must accept work"
        );
    }
    assert!(matches!(
        channels.host_tx.try_send(host_work(HOST_CAPACITY)),
        Err(tokio::sync::mpsc::error::TrySendError::Full(
            HostWork::Probe { index, .. }
        )) if index == HOST_CAPACITY
    ));
    for index in 0..HOST_CAPACITY {
        assert!(matches!(
            host_rx.try_recv(),
            Ok(HostWork::Probe { index: actual, .. }) if actual == index
        ));
    }
}
