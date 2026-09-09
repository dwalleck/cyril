# Spec: Five bundled Cyril palettes

## Request (verbatim)

> implement cyril-fkke first

## Status

Interrogation in progress. No specification or design approval recorded; production implementation has not started.

## What this is

Add Cyril Light, High Contrast Dark, High Contrast Light, Catppuccin Mocha and Gruvbox Dark to the existing bundled theme resolver. Keep startup defaults and selection/configuration unchanged; cyril-qaq0 follows separately.

## Roles

- **Terminal operator**: receives readable, consistent conversation, chrome and syntax presentation when a bundled palette is selected by the later activation change.
- **UI contributor**: obtains a complete resolved theme through the existing theme/mode interface without per-palette rendering branches.

## Established behavior

### Complete palette resolution
- **Given**: one of the five new bundled identifiers and an explicit terminal color mode.
- **When**: the existing resolver is called.
- **Then**: all 31 semantic colors are populated; colored modes select an available syntax component, and no-color emits Reset for every role with no syntax coloring.

### Projection and speaker identity
- **Given**: the same bundled identifier and color mode on repeated calls.
- **When**: it is resolved.
- **Then**: results are identical; ANSI-256 retains the existing fixed-palette projection and ANSI-16 retains the distinct protected speaker slots and muted-family exclusions.

### Existing default
- **Given**: ordinary startup without the future activation feature.
- **When**: Cyril starts.
- **Then**: it still uses Cyril Dark/TrueColor with unchanged accepted rendering.

## Open-question queue

1. Readability policy: recommend primary text/accents >=4.5:1 and muted/status text >=3:1 for ordinary new palettes on their intended painted surfaces; high-contrast variants >=7:1 for primary text/accents and >=4.5:1 for muted/status text. Prefer these targets over exact branded color fidelity when the two conflict. Existing Cyril Dark contract remains unchanged. No existing approval establishes high-contrast or light-background targets.
2. Presentation prerequisite: a new RGB canvas is not currently painted by the frame renderer. Resolve whether the minimal canvas painting needed for real light-theme presentation belongs in this change, without folding in cyril-nx1q's wider role rebinding.
3. Syntax choice: select available compatible light/dark syntax components rather than assume Syntect ships branded components; pin intended syntax readability scope and disclose approximation.

## Related issues and prior art

- cyril-fkke: five approved names, complete roles/syntax, readable intended truecolor presentation and deterministic limited-color projections.
- cyril-ixua: semantic theme substrate, closed; excluded the five palettes and did not approve their RGB mappings.
- cyril-leiq / ADR 0007: established tiered contrast policy; explicitly requires additional themes to extend checks.
- cyril-q9dx: protected ANSI-16 speaker identities; new palettes must satisfy it.
- cyril-nrnq: expanded current contract to 31 roles.
- cyril-qaq0: activation follows palette implementation, with session-only selection and no writes.
- cyril-x5xi: cache identity structural work, not added to this scope absent an actual palette rendering defect.
- cyril-nx1q: broader semantic role binding, not implicitly included.
- cyril-d43s: syntax modifier/background fidelity, distinct from choosing an available syntax component.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| Which issue precedes activation? | cyril-fkke | Request quoted above. | Implement palettes separately before resuming qaq0. |
| Add selection/configuration? | No | cyril-fkke description; current request chooses prerequisite first. | No startup preference, picker or persistence changes. |
| How many semantic colors? | All 31 | Current SourceTheme and nrnq contract. | No stale 19/29-role inventory. |
| Change Cyril Dark? | No | Existing accepted baseline and separate palette addition scope. | Existing default appearance remains protected. |

## Out of scope

Startup selection, environment detection, /theme behavior, persistence, arbitrary custom palettes, wider semantic rebinding, and speculative cache redesign. These remain in their verified covering issues above.

## Approval

Not requested yet: open-question queue must be resolved before full specification sign-off. The request authorizes working on the issue, not unstated palette policy.
