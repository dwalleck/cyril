#!/usr/bin/env python3
import json
import subprocess
import tempfile
from pathlib import Path

bad = r'''
use std::{rc::Rc, thread};
fn main() {
    let state = Rc::new(String::from("domain"));
    thread::spawn(move || println!("{state}"));
}
'''
good = r'''
use std::{cell::RefCell, error::Error, io, rc::Rc, sync::mpsc, thread};
fn main() -> Result<(), Box<dyn Error>> {
    let state = Rc::new(RefCell::new(Vec::<String>::new()));
    let (tx, rx) = mpsc::sync_channel::<String>(1);
    let actor = thread::spawn(move || {
        tx.send(String::from("bounded-handoff"))
            .map_err(|error| io::Error::other(format!("bounded handoff send failed: {error}")))
    });
    state.borrow_mut().push(rx.recv()?);
    actor
        .join()
        .map_err(|_| io::Error::other("bounded handoff actor panicked"))??;
    println!("{}", state.borrow().join(","));
    Ok(())
}
'''
closed_receiver = r'''
use std::{error::Error, io, sync::mpsc};
fn main() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = mpsc::sync_channel::<String>(1);
    drop(rx);
    tx.send(String::from("closed-receiver"))
        .map_err(|error| io::Error::other(format!("closed receiver channel send failed: {error}")))?;
    Ok(())
}
'''

with tempfile.TemporaryDirectory(prefix="cyril-e1-oracle-") as raw_dir:
    directory = Path(raw_dir)
    bad_path = directory / "bad.rs"
    good_path = directory / "good.rs"
    closed_path = directory / "closed.rs"
    bad_path.write_text(bad)
    good_path.write_text(good)
    closed_path.write_text(closed_receiver)
    bad_result = subprocess.run(
        ["rustc", "--edition=2024", str(bad_path), "-o", str(directory / "bad")],
        text=True,
        capture_output=True,
        check=False,
    )
    good_result = subprocess.run(
        ["rustc", "--edition=2024", str(good_path), "-o", str(directory / "good")],
        text=True,
        capture_output=True,
        check=False,
    )
    closed_compile_result = subprocess.run(
        [
            "rustc",
            "--edition=2024",
            str(closed_path),
            "-o",
            str(directory / "closed"),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    if good_result.returncode != 0:
        raise SystemExit(good_result.stderr)
    if closed_compile_result.returncode != 0:
        raise SystemExit(closed_compile_result.stderr)
    run_result = subprocess.run(
        [str(directory / "good")], text=True, capture_output=True, check=False
    )
    closed_run_result = subprocess.run(
        [str(directory / "closed")], text=True, capture_output=True, check=False
    )

result = {
    "claim_ids": ["C1"],
    "moving_rc_across_send_boundary_compiles": bad_result.returncode == 0,
    "rc_send_fence_compile_failed": bad_result.returncode != 0,
    "negative_control_mentions_send": "cannot be sent between threads safely" in bad_result.stderr,
    "bounded_message_handoff_with_local_rc_compiles": good_result.returncode == 0,
    "bounded_message_handoff_output": run_result.stdout.strip(),
    "bounded_message_handoff_run_succeeded": run_result.returncode == 0,
    "closed_receiver_compiles": closed_compile_result.returncode == 0,
    "closed_receiver_runtime_failed": closed_run_result.returncode != 0,
    "closed_receiver_diagnostic": closed_run_result.stderr.strip(),
    "closed_receiver_error_is_contextual": (
        "closed receiver channel send failed" in closed_run_result.stderr
    ),
    "closed_receiver_error_mentions_channel": (
        "sending on a closed channel" in closed_run_result.stderr
    ),
    "closed_receiver_error_no_panic": "panicked" not in closed_run_result.stderr,
    "unsafe_used": False,
    "shared_lock_used_for_domain_state": False,
}
print(json.dumps(result, indent=2, sort_keys=True))
if not (
    result["rc_send_fence_compile_failed"]
    and result["negative_control_mentions_send"]
    and result["bounded_message_handoff_with_local_rc_compiles"]
    and result["bounded_message_handoff_run_succeeded"]
    and result["bounded_message_handoff_output"] == "bounded-handoff"
    and result["closed_receiver_compiles"]
    and result["closed_receiver_runtime_failed"]
    and result["closed_receiver_error_is_contextual"]
    and result["closed_receiver_error_mentions_channel"]
    and result["closed_receiver_error_no_panic"]
):
    raise SystemExit(1)
