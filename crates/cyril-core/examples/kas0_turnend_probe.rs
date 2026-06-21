//! KAS-0 cheapest-falsifier (cyril-atjw) — gilfoyle prove-it-prototype.
//!
//! Models the proposed single-mediator loop (ADR-0004): the off-loop "prompt
//! task" and the "notification source" both feed ONE internal channel; the loop
//! `select!`s over commands + that channel, forwards to the App, and clears a
//! loop-local `turn_in_flight` flag by OBSERVING `TurnCompleted` (D2/D3/D4) —
//! instead of keying the busy-guard off `prompt_task.is_finished()`.
//!
//! Independent oracle = the CURRENT bridge invariants encoded in the FakeAgent
//! harness (bridge.rs:1441-1549): exactly one `TurnCompleted` per turn, a
//! mid-turn command processed BEFORE it (loop free during a turn), and a
//! concurrent prompt rejected by the busy-guard. If this probe reproduces those
//! invariants under the new mechanism, D2/D3/D4 hold; if not, the design is
//! falsified for ~90 lines instead of after 5 slices.
//!
//! Caveat: this validates the concurrency SHAPE in isolation, not the real ACP
//! plumbing — Slice 2's FakeAgent harness on the actual bridge is the integration
//! oracle. Run:  cargo run -p cyril-core --example kas0_turnend_probe

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::mpsc;

enum Command {
    SendPrompt(u64),
    Probe,
    Shutdown,
}

#[derive(Clone, PartialEq, Debug)]
enum Note {
    Chunk(u64),
    TurnCompleted(u64),
}

#[derive(Clone, PartialEq, Debug)]
enum Event {
    Forwarded(Note),  // loop forwarded a notification to the App
    Probe,            // mid-turn command processed by the loop (loop was free)
    Accepted(u64),    // SendPrompt started a turn
    Rejected(u64),    // SendPrompt refused by the busy-guard
    FlagCleared(u64), // loop cleared turn_in_flight on observing TurnCompleted
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    match tokio::time::timeout(Duration::from_secs(5), run()).await {
        Ok(log) => assert_oracle(&log),
        Err(_) => panic!("DEADLOCK: probe did not complete in 5s — DESIGN FALSIFIED"),
    }
}

async fn run() -> Vec<Event> {
    tokio::task::LocalSet::new().run_until(loop_body()).await
}

async fn loop_body() -> Vec<Event> {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(16);
    let (inb_tx, mut inb_rx) = mpsc::channel::<Note>(16);
    let log: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
    let mut turn_in_flight: Option<u64> = None;

    // Driver: scripts the sequence, injecting commands MID-TURN (before turn A's
    // prompt task emits TurnCompleted at +50ms). A failed send means the loop is
    // already gone, so the driver simply stops.
    {
        let cmd_tx = cmd_tx.clone();
        tokio::task::spawn_local(async move {
            let script = [
                Command::SendPrompt(1), // turn A
                Command::Probe,         // mid-turn command
                Command::SendPrompt(2), // concurrent -> reject
            ];
            for c in script {
                if cmd_tx.send(c).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(150)).await; // let turn A drain
            if cmd_tx.send(Command::SendPrompt(3)).await.is_err() {
                return; // turn B (flag clear?)
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
            let _stop = cmd_tx.send(Command::Shutdown).await;
        });
    }

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => match cmd {
                Command::SendPrompt(sid) => {
                    if turn_in_flight.is_some() {
                        log.borrow_mut().push(Event::Rejected(sid)); // busy-guard (was is_finished())
                    } else {
                        turn_in_flight = Some(sid);
                        log.borrow_mut().push(Event::Accepted(sid));
                        // off-loop prompt task: synthesize TurnCompleted onto the
                        // SAME inbound channel the notification source uses (v2 shape).
                        let inb = inb_tx.clone();
                        tokio::task::spawn_local(async move {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            if inb.send(Note::Chunk(sid)).await.is_err() {
                                return;
                            }
                            let _done = inb.send(Note::TurnCompleted(sid)).await;
                        });
                    }
                }
                Command::Probe => log.borrow_mut().push(Event::Probe),
                Command::Shutdown => break,
            },
            Some(note) = inb_rx.recv() => {
                log.borrow_mut().push(Event::Forwarded(note.clone())); // forward to App
                if let Note::TurnCompleted(sid) = note {
                    turn_in_flight = None; // CLEAR by observing the marker (D3)
                    log.borrow_mut().push(Event::FlagCleared(sid));
                }
            }
        }
    }

    log.borrow().clone()
}

fn assert_oracle(log: &[Event]) {
    // Index of an expected event, or a falsification panic if it never happened.
    let require = |e: &Event| -> usize {
        match log.iter().position(|x| x == e) {
            Some(i) => i,
            None => panic!("DESIGN FALSIFIED — expected event never occurred: {e:?}"),
        }
    };
    let count_tc = log
        .iter()
        .filter(|e| matches!(e, Event::Forwarded(Note::TurnCompleted(_))))
        .count();

    // Oracle 1: exactly one TurnCompleted per accepted turn (2 turns accepted).
    assert_eq!(count_tc, 2, "expected one TurnCompleted per accepted turn");
    require(&Event::Accepted(1));
    require(&Event::Accepted(3));

    // Oracle 2: the mid-turn Probe command is processed BEFORE turn A's
    // TurnCompleted (the loop stays free during a turn — bridge.rs:1526 baseline).
    let probe_at = require(&Event::Probe);
    let tc1_at = require(&Event::Forwarded(Note::TurnCompleted(1)));
    assert!(
        probe_at < tc1_at,
        "mid-turn command must be processed before TurnCompleted (loop not free!)"
    );

    // Oracle 3: the busy-guard rejects a concurrent prompt while a turn is in flight.
    require(&Event::Rejected(2));

    // Oracle 4: the flag CLEARS on observing TurnCompleted, so the next turn is
    // accepted — and only after the clear (proves the clear, not a lucky race).
    let cleared1_at = require(&Event::FlagCleared(1));
    let accepted3_at = require(&Event::Accepted(3));
    assert!(
        cleared1_at < accepted3_at,
        "turn B must be accepted only AFTER the flag cleared"
    );

    println!("CHEAPEST-FALSIFIER PASSED — probe reproduces all 4 oracle invariants:");
    println!("  1. exactly one TurnCompleted per turn ({count_tc} turns)");
    println!("  2. mid-turn command processed before TurnCompleted (loop free)");
    println!("  3. busy-guard rejects concurrent prompt (flag, not is_finished())");
    println!("  4. flag clears on observing TurnCompleted -> next turn accepted");
    println!("\nD2/D3/D4 hold. Budgeted-plan gate satisfied.");
}
