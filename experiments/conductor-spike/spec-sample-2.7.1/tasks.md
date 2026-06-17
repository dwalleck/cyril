# Implementation Plan: csv2json

## Overview

Implement a CLI tool that reads a CSV file and outputs a JSON array of row objects. The implementation follows a pipeline architecture with three discrete modules: CLI (argument parsing + I/O), Parser (CSV string to structured data), and Serializer (structured data to JSON string). TypeScript with Node.js, tested with vitest and fast-check.

## Tasks

- [ ] 1. Set up project structure and core interfaces
  - [ ] 1.1 Initialize Node.js/TypeScript project
    - Create `package.json` with project metadata and scripts (`build`, `test`)
    - Create `tsconfig.json` targeting Node.js with strict mode enabled
    - Install dependencies: `typescript`, `vitest`, `fast-check`
    - Create directory structure: `src/`, `tests/`
    - _Requirements: 1.1, 2.1_

  - [ ] 1.2 Define error types and shared interfaces
    - Create `src/errors.ts` with `CsvParseError` class extending `Error`
    - Create `src/types.ts` with `ParseResult` interface (`headers: string[]`, `rows: string[][]`) and `SerializeOptions` interface (`pretty: boolean`)
    - _Requirements: 1.4, 1.5, 1.6_

- [ ] 2. Implement CSV Parser
  - [ ] 2.1 Implement core CSV parsing logic
    - Create `src/parser.ts` exporting `parseCsv(input: string): ParseResult`
    - Handle empty/whitespace-only input detection (throw `CsvParseError`)
    - Parse header row as first line
    - Parse data rows with field splitting on commas
    - Support quoted fields: remove enclosing quotes, handle embedded commas, handle escaped double quotes (`""` → `"`), handle embedded newlines (LF, CR, CRLF)
    - Preserve leading/trailing whitespace in unquoted fields
    - Treat unquoted mid-field double quotes as literal content
    - Detect unclosed quotes and throw `CsvParseError` with line number
    - Validate column counts: pad short rows with empty strings, throw error for rows with too many fields
    - _Requirements: 1.2, 2.6, 2.7, 4.2, 4.3, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

  - [ ]* 2.2 Write property test: CSV Parse Round-Trip
    - **Property 1: CSV Parse Round-Trip**
    - Generate random strings (with commas, quotes, newlines, whitespace), CSV-encode them, parse with `parseCsv`, verify the recovered value matches the original
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5, 5.7**

  - [ ]* 2.3 Write property test: Structural Integrity
    - **Property 2: Structural Integrity**
    - Generate random headers (M columns) and rows (N data rows), build a valid CSV string, parse it, verify the result has N rows each with M fields and headers match
    - **Validates: Requirements 1.2, 2.1, 2.3, 6.1, 6.3**

  - [ ]* 2.4 Write property test: Row Order Preservation
    - **Property 3: Row Order Preservation**
    - Generate multi-row CSVs with distinguishable rows, parse, verify output order matches input order
    - **Validates: Requirements 2.2**

  - [ ]* 2.5 Write property test: Short Row Padding
    - **Property 4: Short Row Padding**
    - Generate CSVs where data rows have fewer fields than headers, parse, verify missing fields are padded with empty strings
    - **Validates: Requirements 2.6**

  - [ ]* 2.6 Write unit tests for parser edge cases
    - Test empty file (0 bytes) → error
    - Test whitespace-only file → error
    - Test header only, no data rows → empty rows array
    - Test unclosed quote → parse error with line number
    - Test extra columns → error
    - Test quoted field with embedded comma, newline, escaped quotes
    - Test unquoted field with leading/trailing whitespace preserved
    - Test unquoted field with mid-field double quote treated as literal
    - _Requirements: 4.2, 4.3, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

- [ ] 3. Implement JSON Serializer
  - [ ] 3.1 Implement serialization logic
    - Create `src/serializer.ts` exporting `serialize(headers: string[], rows: string[][], options: SerializeOptions): string`
    - Build array of objects from headers and rows (keys = headers, values = row cells as strings, missing trailing fields = `""`)
    - Compact mode: no whitespace between tokens (use `JSON.stringify` with no spacing)
    - Pretty mode: 2-space indentation (use `JSON.stringify` with 2-space indent)
    - Always append a trailing newline character
    - Empty rows array → output `[]\n` (both modes)
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3, 4.1, 4.4_

  - [ ]* 3.2 Write property test: Serialization Round-Trip
    - **Property 5: Serialization Round-Trip**
    - Generate valid (headers, rows) data, serialize to JSON, parse with `JSON.parse`, verify values match original row data
    - **Validates: Requirements 6.2, 2.4**

  - [ ]* 3.3 Write property test: Output Formatting Correctness
    - **Property 6: Output Formatting Correctness**
    - Generate valid data, serialize in compact mode and verify no whitespace between tokens (outside strings), serialize in pretty mode and verify 2-space indentation, verify both modes end with exactly one trailing newline
    - **Validates: Requirements 3.1, 3.2, 3.3**

  - [ ]* 3.4 Write unit tests for serializer edge cases
    - Test compact output has no whitespace between tokens
    - Test pretty output has 2-space indentation
    - Test trailing newline present in both modes
    - Test empty rows → `[]\n`
    - Test empty rows + pretty → `[]\n`
    - Test all values are strings including empty cells
    - _Requirements: 2.4, 3.1, 3.2, 3.3, 4.1, 4.4_

- [ ] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Implement CLI Layer
  - [ ] 5.1 Implement CLI argument parsing and orchestration
    - Create `src/cli.ts` with `main(args: string[]): void` function
    - Parse positional argument (file path) and optional `--pretty` flag
    - If file path missing: print usage message `Usage: csv2json <file> [--pretty]` to stderr, exit 1
    - Read file with `fs.readFileSync` (UTF-8)
    - Handle `ENOENT` → `Error: File not found: <path>` to stderr, exit 1
    - Handle `EACCES` → `Error: Cannot read file: <path>` to stderr, exit 1
    - Handle other read errors → generic error message to stderr, exit 1
    - Call `parseCsv` and catch `CsvParseError` → print message to stderr, exit 1
    - Call `serialize` with parsed data and pretty option
    - Write result to stdout, exit 0
    - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.6, 2.5_

  - [ ] 5.2 Create executable entry point
    - Create `src/index.ts` that imports and calls `main` with `process.argv.slice(2)`
    - Add `bin` field to `package.json` pointing to compiled output
    - Add build script to compile TypeScript to JavaScript
    - _Requirements: 1.1_

  - [ ]* 5.3 Write integration tests for CLI layer
    - Test missing argument → usage message on stderr, exit 1
    - Test non-existent file → file not found error on stderr, exit 1
    - Test valid CSV → correct JSON on stdout, exit 0
    - Test `--pretty` flag → indented output
    - Test empty file → error on stderr, exit 1
    - Test permission denied → error on stderr, exit 1
    - _Requirements: 1.3, 1.4, 1.5, 1.6, 2.5, 3.1, 3.2_

- [ ] 6. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples and edge cases
- The parser and serializer are pure functions for easy testing without file system access
- The CLI layer handles all I/O concerns and delegates to pure functions

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["1.2"] },
    { "id": 2, "tasks": ["2.1", "3.1"] },
    { "id": 3, "tasks": ["2.2", "2.3", "2.4", "2.5", "2.6", "3.2", "3.3", "3.4"] },
    { "id": 4, "tasks": ["5.1"] },
    { "id": 5, "tasks": ["5.2", "5.3"] }
  ]
}
```
