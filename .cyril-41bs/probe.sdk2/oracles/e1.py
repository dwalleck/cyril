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
use std::{cell::RefCell, rc::Rc, sync::mpsc, thread};
fn main() {
    let state = Rc::new(RefCell::new(Vec::<String>::new()));
    let (tx, rx) = mpsc::sync_channel::<String>(1);
    let actor = thread::spawn(move || tx.send(String::from("bounded-handoff")).expect("send"));
    state.borrow_mut().push(rx.recv().expect("receive"));
    actor.join().expect("join");
    println!("{}", state.borrow().join(","));
}
'''

with tempfile.TemporaryDirectory(prefix="cyril-e1-oracle-") as raw_dir:
    directory = Path(raw_dir)
    bad_path = directory / "bad.rs"
    good_path = directory / "good.rs"
    bad_path.write_text(bad)
    good_path.write_text(good)
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
    if good_result.returncode != 0:
        raise SystemExit(good_result.stderr)
    run_result = subprocess.run(
        [str(directory / "good")], text=True, capture_output=True, check=True
    )

print(
    json.dumps(
        {
            "claim_ids": ["C1"],
            "moving_rc_across_send_boundary_compiles": bad_result.returncode == 0,
            "negative_control_mentions_send": "cannot be sent between threads safely" in bad_result.stderr,
            "bounded_message_handoff_with_local_rc_compiles": good_result.returncode == 0,
            "bounded_message_handoff_output": run_result.stdout.strip(),
            "unsafe_used": False,
            "shared_lock_used_for_domain_state": False,
        },
        indent=2,
        sort_keys=True,
    )
)
