/**
 * Core type definitions for csv2json CLI tool.
 */

/** Parsed CLI arguments */
export interface CliOptions {
  filePath: string;
  pretty: boolean;
}

/** Successful CSV parse result containing headers and data rows */
export interface ParseResult {
  headers: string[];
  rows: string[][];
}

/** Error types that can occur during CSV parsing */
export type ParseErrorType = 'duplicate_header' | 'empty_header' | 'unterminated_quote';

/** Error returned when CSV parsing fails */
export interface ParseError {
  type: ParseErrorType;
  message: string;
  detail?: string; // e.g., the duplicate column name
}
