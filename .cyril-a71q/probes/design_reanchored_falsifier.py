"""Cheapest falsifier for the re-anchored cyril-a71q design (spec.md 2026-07-12).

Models the mediator policy: TurnId stamping at dispatch, first-source-wins release,
absorb-first one-entry companion ledger, id-match for synthesized completions,
scoped-session match for identity-free wire turn_end, foreign routing, fail-stop.

Expected dispositions are HARDCODED literals transcribed from spec.md behavior
sections (the independent oracle); hidden turn labels exist only in this harness.
Mutations inject concrete buggy policies; each must fail a non-empty, pairwise
distinct set of assertions while the correct policy fails none.
"""

KAS, V2 = "kas", "v2"


class Mediator:
    def __init__(self, session_only=False, no_ledger=False, release_first=False,
                 v2_session_match=False):
        self.flags = (session_only, no_ledger, release_first, v2_session_match)
        self.next_id = 0
        self.active = None            # {label, owner, session, engine}
        self.expected = None          # ("id", owner, label) | ("wire", session, owner, label)
        self.completions = []         # (label, source, reason) forwarded to main
        self.routed_foreign = []      # (session, reason)
        self.evidence = {}            # label -> [(source, reason)]
        self.failed = False
        self.lifecycle = []

    def accept(self, label, session, engine):
        if self.active or self.failed:
            return False
        self.active = dict(label=label, owner=self.next_id, session=session, engine=engine)
        self.next_id += 1
        return True

    def _record(self, label, source, reason):
        self.evidence.setdefault(label, []).append((source, reason))

    def _release(self, source, reason):
        t = self.active
        self.completions.append((t["label"], source, reason))
        self._record(t["label"], source, reason)
        no_ledger = self.flags[1]
        if t["engine"] == KAS and not no_ledger:
            other = ("id", t["owner"], t["label"]) if source == "turn_end" \
                else ("wire", t["session"], t["owner"], t["label"])
            self.expected = other
        self.active = None

    def synth(self, owner, label, reason):
        session_only, _, _, v2_session_match = (
            self.flags[0], self.flags[1], self.flags[2], self.flags[3])
        if self.expected and self.expected[0] == "id" and self.expected[1] == owner:
            self._record(self.expected[2], "response", reason)
            self.expected = None
            return "absorbed"
        if self.active:
            if v2_session_match:
                return "dropped"  # global has session None: never matches -> freeze
            if session_only or self.active["owner"] == owner:
                self._release("response", reason)
                return "released"
        return "dropped"

    def turn_end(self, session, reason):
        session_only, no_ledger, release_first, _ = self.flags
        exp = self.expected
        exp_hit = exp and exp[0] == "wire" and exp[1] == session
        act_hit = self.active and self.active["session"] == session \
            and self.active["engine"] == KAS
        if self.active and self.active["session"] != session:
            if not act_hit and not exp_hit:
                self.routed_foreign.append((session, reason))
                return "routed"
        order = ["active", "expected"] if release_first else ["expected", "active"]
        for rule in order:
            if rule == "expected" and exp_hit and not no_ledger:
                self._record(exp[3], "turn_end", reason)
                self.expected = None
                return "absorbed"
            if rule == "active" and act_hit:
                self._release("turn_end", reason)
                return "released"
        if not self.active and session is not None and not exp_hit:
            self.routed_foreign.append((session, reason)) if session != "S" else None
        return "dropped"

    def fail_stop(self, reason):
        t = self.active
        self.lifecycle += ["BridgeError", "TurnCompleted", "BridgeDisconnected"]
        if t:
            self.completions.append((t["label"], "failure", reason))
        self.active = None
        self.failed = True


def run_traces(**flags):
    fails = []

    def check(name, cond):
        if not cond:
            fails.append(name)

    # T1 normal researched order: turn_end then response
    m = Mediator(**flags)
    m.accept("A", "S", KAS)
    check("T1.release_on_turn_end", m.turn_end("S", "end_turn") == "released")
    check("T1.companion_absorbed", m.synth(0, "A", "end_turn") == "absorbed")
    check("T1.one_completion", [c[0] for c in m.completions] == ["A"])
    check("T1.both_evidence", set(m.evidence.get("A", [])) ==
          {("turn_end", "end_turn"), ("response", "end_turn")})
    check("T1.B_accepted_after", m.accept("B", "S", KAS))

    # T2 inverted receipt order: response then turn_end
    m = Mediator(**flags)
    m.accept("A", "S", KAS)
    check("T2.release_on_response", m.synth(0, "A", "cancelled") == "released")
    check("T2.companion_absorbed", m.turn_end("S", "cancelled") == "absorbed")
    check("T2.one_completion", [c[0] for c in m.completions] == ["A"])
    check("T2.both_evidence", set(m.evidence.get("A", [])) ==
          {("response", "cancelled"), ("turn_end", "cancelled")})

    # T3 the original a71q residual: A's late stamped completion during B
    m = Mediator(**flags)
    m.accept("A", "S", KAS)
    m.turn_end("S", "end_turn")           # A releases; expects A's synth
    m.accept("B", "S", KAS)               # B active before A's synth arrives
    check("T3.stale_stamped_no_release", m.synth(0, "A", "end_turn") != "released")
    check("T3.B_still_busy", m.active is not None and m.active["label"] == "B")
    m.turn_end("S", "end_turn")           # B's own turn_end
    check("T3.each_turn_once", [c[0] for c in m.completions] == ["A", "B"])

    # T4 superseded-design History 1: A released via response; late A turn_end during B
    m = Mediator(**flags)
    m.accept("A", "S", KAS)
    m.synth(0, "A", "end_turn")           # response wins; expects A's wire turn_end
    m.accept("B", "S", KAS)
    check("T4.ambiguous_absorbed", m.turn_end("S", "end_turn") == "absorbed")
    check("T4.B_not_wrongly_released", m.active is not None and m.active["label"] == "B")
    check("T4.B_releases_via_own_synth", m.synth(1, "B", "end_turn") == "released")
    check("T4.safety", [c[0] for c in m.completions] == ["A", "B"])

    # T5 History 2 (double drift): same visible prefix as T4; B's synth never arrives
    m = Mediator(**flags)
    m.accept("A", "S", KAS)
    m.synth(0, "A", "end_turn")
    m.accept("B", "S", KAS)
    act = m.turn_end("S", "end_turn")     # identical visible frame as T4
    check("T5.same_action_as_T4", act == "absorbed")
    check("T5.signed_residual_busy", m.active is not None)  # until lifecycle/fail-stop
    m.fail_stop("agent death")
    check("T5.lifecycle_order", m.lifecycle ==
          ["BridgeError", "TurnCompleted", "BridgeDisconnected"])
    check("T5.no_prompt_after_failstop", not m.accept("C", "S", KAS))

    # T6 cross-session scoped completion during main B
    m = Mediator(**flags)
    m.accept("B", "S", KAS)
    check("T6.foreign_routed", m.turn_end("X", "end_turn") == "routed")
    check("T6.main_untouched", m.active is not None and not m.completions)
    check("T6.routed_once", m.routed_foreign == [("X", "end_turn")])

    # T7 v1/v2 global id-match; stale stamped drop; no freeze
    m = Mediator(**flags)
    m.accept("A", None, V2)
    check("T7.v2_releases_on_id", m.synth(0, "A", "end_turn") == "released")
    check("T7.v2_not_frozen", m.accept("B", None, V2))
    check("T7.stale_stamped_drops", m.synth(0, "A", "end_turn") == "dropped")
    check("T7.B_still_busy", m.active is not None and m.active["label"] == "B")

    return fails


def main():
    correct = run_traces()
    print(f"REANCHOR correct_policy_failures={correct}")
    mutations = {
        "M1_session_only": dict(session_only=True),
        "M2_no_ledger": dict(no_ledger=True),
        "M3_release_first": dict(release_first=True),
        "M4_v2_session_match": dict(v2_session_match=True),
    }
    signatures = {}
    for name, f in mutations.items():
        failed = run_traces(**f)
        signatures[name] = tuple(sorted(failed))
        print(f"REANCHOR {name} failed={sorted(failed)}")
    ok = (not correct
          and all(signatures.values())
          and len(set(signatures.values())) == len(signatures))
    print("REANCHOR distinct_mutation_signatures="
          f"{len(set(signatures.values()))}/{len(signatures)}")
    print("REANCHORED-CHEAPEST-" + ("PASSED" if ok else "FAILED"))


if __name__ == "__main__":
    main()
