# cyril-tpfd — issues to file at close-out (jsonl stays off this branch;
# parallel sessions are active)

1. NOT-an-issue (fix in this branch instead): the per-release hooks fence
   `.cyril-jiyn/probe-hooks-ab-2.13.0.py` has the profileArn-object bug
   (sends the profile row verbatim; row is `{"arn", ...}` JSON object) —
   its turns can never complete. Fix `token()` there + annotate the jiyn
   A/B result caveat in `experiments/conductor-spike/README.md` fence
   note if needed.
2. CANDIDATE issue: unbriefed-model hook authority — with cyril's
   clientInfo, KAS omits its hooks briefing and the model may refuse
   injected HOOK_INSTRUCTION content (observed live 2026-07-23:
   "not a legitimate system directive"). Mitigation candidate per
   cyril-0wyn triage: cyril injects its own corrected hooks briefing via
   steering/context when hooks are configured. Search rivets for an
   existing 0wyn-followup issue before filing new.
