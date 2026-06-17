# Requirements Document

## Introduction

`csv2json` is a command-line tool that reads a CSV file and converts it into a JSON array of row objects. Each row in the CSV becomes a JSON object where column headers serve as keys. The tool supports a `--pretty` flag for human-readable formatted output.

## Glossary

- **CLI**: The command-line interface through which users invoke the csv2json tool
- **CSV_File**: A comma-separated values file with a header row followed by zero or more data rows
- **Header_Row**: The first row of the CSV_File, containing column names used as JSON object keys
- **Row_Object**: A JSON object representing a single CSV data row, with Header_Row values as keys and cell values as string values
- **JSON_Array**: The output array containing one Row_Object per data row in the CSV_File
- **Pretty_Flag**: The `--pretty` command-line option that enables indented, human-readable JSON output
- **Parser**: The component responsible for reading and interpreting CSV_File content
- **Serializer**: The component responsible for converting parsed data into JSON output

## Requirements

### Requirement 1: Read and Parse CSV Input

**User Story:** As a user, I want to provide a CSV file path as an argument, so that the tool can read and parse its contents.

#### Acceptance Criteria

1. WHEN a file path is provided as the first positional argument, THE CLI SHALL read the file at the specified path
2. WHEN the CSV_File contains a Header_Row and data rows, THE Parser SHALL parse each data row into a Row_Object using Header_Row values as keys
3. IF the file path argument is missing, THEN THE CLI SHALL print a usage message to standard error and exit with a non-zero exit code
4. IF the specified file does not exist, THEN THE CLI SHALL print an error message indicating the file was not found to standard error and exit with a non-zero exit code
5. IF the specified file is not valid CSV (malformed quoting, inconsistent column counts across rows, or non-text binary content), THEN THE CLI SHALL print an error message indicating a parse failure to standard error and exit with a non-zero exit code
6. IF the specified file cannot be read due to insufficient permissions, THEN THE CLI SHALL print an error message indicating the file is not readable to standard error and exit with a non-zero exit code

### Requirement 2: Produce JSON Array Output

**User Story:** As a user, I want the tool to output a JSON array of row objects, so that I can use the converted data in JSON-consuming applications.

#### Acceptance Criteria

1. WHEN the CSV_File is successfully parsed, THE Serializer SHALL produce a valid JSON_Array containing one Row_Object per data row
2. THE Serializer SHALL preserve the order of rows from the CSV_File in the JSON_Array
3. THE Serializer SHALL use Header_Row values as keys in each Row_Object
4. THE Serializer SHALL represent all cell values as JSON strings, including empty cells which SHALL be represented as empty strings (`""`)
5. WHEN the JSON_Array is successfully serialized, THE CLI SHALL write the JSON_Array to standard output and exit with code 0
6. IF a data row contains fewer fields than the Header_Row, THEN THE Serializer SHALL assign an empty string (`""`) as the value for each missing trailing field
7. IF a data row contains more fields than the Header_Row, THEN THE CLI SHALL print an error message indicating a column count mismatch to standard error and exit with a non-zero exit code

### Requirement 3: Pretty Print Option

**User Story:** As a user, I want a `--pretty` flag, so that I can produce indented JSON output for readability.

#### Acceptance Criteria

1. WHEN the Pretty_Flag is provided, THE Serializer SHALL output the JSON_Array with 2-space indentation and a newline character after each bracket, brace, colon-value pair, and array element
2. WHEN the Pretty_Flag is not provided, THE Serializer SHALL output the JSON_Array with no whitespace between tokens except within string values
3. THE Serializer SHALL terminate the JSON output with a single trailing newline character

### Requirement 4: Handle Empty CSV Files

**User Story:** As a user, I want predictable output when the CSV file has no data rows, so that downstream tooling handles edge cases gracefully.

#### Acceptance Criteria

1. WHEN the CSV_File contains only a Header_Row and no data rows, THE Serializer SHALL output an empty JSON_Array (`[]`) to standard output
2. IF the CSV_File is completely empty (zero bytes), THEN THE CLI SHALL print an error message indicating the file is empty to standard error and exit with a non-zero exit code
3. IF the CSV_File contains only whitespace characters (spaces, tabs, or newline characters) and no printable content, THEN THE CLI SHALL treat the file as empty, print an error message indicating the file is empty to standard error, and exit with a non-zero exit code
4. WHEN the CSV_File contains only a Header_Row and no data rows and the Pretty_Flag is provided, THE Serializer SHALL output an empty JSON_Array (`[]`) with no additional whitespace or newlines inside the brackets

### Requirement 5: CSV Parsing Correctness

**User Story:** As a user, I want the parser to correctly handle standard CSV features, so that quoted fields and special characters are converted accurately.

#### Acceptance Criteria

1. WHEN a cell value is enclosed in double quotes, THE Parser SHALL remove the enclosing quotes and use the inner content as the value
2. WHEN a quoted cell value contains a comma, THE Parser SHALL treat the comma as literal content within that field
3. WHEN a quoted cell value contains escaped double quotes (two consecutive double quotes), THE Parser SHALL replace them with a single double quote in the output
4. WHEN a quoted cell value contains a newline character (LF, CR, or CRLF), THE Parser SHALL preserve the newline as literal content within that field
5. WHEN an unquoted cell value contains leading or trailing whitespace, THE Parser SHALL preserve the whitespace as part of the value
6. IF a cell value contains an opening double quote that is never closed before end of file, THEN THE Parser SHALL report a parse error and THE CLI SHALL exit with a non-zero exit code
7. WHEN an unquoted cell value contains a double quote character that is not at the start of the field, THE Parser SHALL treat the double quote as literal content

### Requirement 6: Round-Trip Property (Parse and Serialize)

**User Story:** As a developer, I want to verify that parsing and serialization are consistent, so that no data is lost or corrupted during conversion.

#### Acceptance Criteria

1. FOR ALL valid CSV_Files with N data rows, THE Serializer SHALL produce a JSON_Array of exactly N Row_Objects where each Row_Object contains exactly the same number of keys as there are columns in the Header_Row
2. FOR ALL valid JSON_Arrays produced by THE Serializer, parsing the JSON output with any standards-compliant JSON parser SHALL produce an array of objects identical in structure and values to the internal Row_Objects
3. FOR ALL Row_Objects produced by THE Parser, the set of keys SHALL equal the set of Header_Row values and the value for each key SHALL be the exact string content of the corresponding cell after quote removal and escape processing
