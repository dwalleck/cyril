# Raw request

Requester invocation, verbatim:

> /ship cyril-vhfz

Tracker request: make Cyril tolerate both observed KAS workflow pause orderings after kiro-cli 2.18.0 / `@kiro/agent` 0.38.7 moved run-level `_kiro/workflow/paused` to immediately before the non-terminal paused `run_complete`. Treat `node_paused` as the immediate pause signal and run-level `paused` as a summary frame. Preserve compatibility with the earlier park-time run-level `paused` ordering.
