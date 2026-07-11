# Separate semantic themes from terminal color modes

Status: accepted (2026-07-10)

Cyril will render through a resolved semantic `Theme` rather than allowing
widgets to choose colors directly. Visual theme selection (`cyril-dark`,
`cyril-light`, high-contrast variants, and bundled aesthetic palettes) is a
separate axis from terminal color capability (`truecolor`, `ansi256`, `ansi16`,
or `none`): the selected theme is projected into the selected color mode once,
then exposed read-only from UI state so live `/theme` preview preserves the
state/renderer boundary. This avoids duplicating themes by terminal capability,
prevents inconsistent widget-level color fallback, and keeps status meaning
available through labels and symbols when color is limited or disabled;
arbitrary user-defined palettes are not supported by this theme model.
