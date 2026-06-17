# Requirements Document

## Introduction

`csv2json` is a command-line tool that reads a CSV file and converts it into a JSON array of objects. Each row in the CSV becomes a JSON object with keys derived from the header row. The tool supports a `--pretty` flag to output human-readable, indented JSON.

## Glossary

- **CLI**: The csv2json command-line interface application
- **CSV_File**: A comma-separated values file with a header row followed by zero or more data rows
- **Header_Row**: The first row of the CSV_File, containing column names used as JSON object keys
- **Data_Row**: Any row in the CSV_File after the Header_Row, containing values for each column
- **JSON_Array**: The output format; an array where each element is an object representing one Data_Row
- **Pretty_Mode**: An output mode activated by the --pretty flag that produces indented, human-readable JSON

## Requirements

### Requirement 1: CSV File Input

**User Story:** As a user, I want to specify a CSV file path as input, so that the tool knows which file to convert.

#### Acceptance Criteria

1. WHEN a file path argument is provided as the first positional argument, THE CLI SHALL read the file at that path as a CSV_File
2. IF the file path does not exist, THEN THE CLI SHALL exit with a non-zero exit code and print an error message to stderr indicating the file was not found
3. IF the file path argument is not provided, THEN THE CLI SHALL exit with a non-zero exit code and print a usage message to stderr showing the expected invocation syntax
4. IF the file path points to a directory or non-regular file, THEN THE CLI SHALL exit with a non-zero exit code and print an error message to stderr indicating that the path is not a regular file
5. IF more than one positional argument is provided, THEN THE CLI SHALL use the first positional argument as the file path and ignore additional positional arguments

### Requirement 2: CSV Parsing

**User Story:** As a user, I want the tool to correctly parse CSV data, so that all fields are accurately represented in the output.

#### Acceptance Criteria

1. WHEN a valid CSV_File is read, THE CLI SHALL use the Header_Row to determine the keys for each JSON object
2. WHEN a valid CSV_File is read, THE CLI SHALL create one JSON object per Data_Row with values mapped to the corresponding Header_Row keys
3. WHEN a CSV_File contains only a Header_Row and no Data_Rows, THE CLI SHALL output an empty JSON_Array
4. WHEN a field in the CSV_File is enclosed in double quotes, THE CLI SHALL treat the content between quotes as a single field value, including any commas, newline characters, and escaped double quotes (represented as two consecutive double quotes) within
5. IF a Data_Row contains fewer fields than the Header_Row, THEN THE CLI SHALL use empty strings for the missing fields
6. IF a Data_Row contains more fields than the Header_Row, THEN THE CLI SHALL ignore the extra fields
7. WHEN a field in the CSV_File is not enclosed in double quotes, THE CLI SHALL preserve the field value exactly as it appears between delimiters, including any leading or trailing whitespace

### Requirement 3: JSON Output

**User Story:** As a user, I want the tool to output a valid JSON array to stdout, so that I can pipe it into other tools or redirect it to a file.

#### Acceptance Criteria

1. THE CLI SHALL write the JSON_Array to stdout
2. THE CLI SHALL produce output that is valid JSON conforming to RFC 8259
3. WHEN the --pretty flag is not provided, THE CLI SHALL output compact JSON with no whitespace between tokens except where required by JSON syntax
4. WHEN the --pretty flag is provided, THE CLI SHALL output JSON indented with 2 spaces per nesting level
5. THE CLI SHALL output all field values as JSON strings
6. THE CLI SHALL terminate output with a single trailing newline character
7. WHEN conversion succeeds, THE CLI SHALL exit with exit code 0

### Requirement 4: Pretty Print Flag

**User Story:** As a user, I want a --pretty flag, so that I can get human-readable JSON output when needed.

#### Acceptance Criteria

1. WHEN the --pretty flag is provided, THE CLI SHALL activate Pretty_Mode
2. WHEN the -p shorthand flag is provided, THE CLI SHALL activate Pretty_Mode
3. WHILE Pretty_Mode is active, THE CLI SHALL output the JSON_Array with each opening bracket, object, and closing bracket on separate lines, indented with 2 spaces per nesting level
4. WHEN neither --pretty nor -p is provided, THE CLI SHALL output compact JSON with no unnecessary whitespace

### Requirement 5: Error Handling

**User Story:** As a user, I want clear error messages, so that I can understand what went wrong when the tool fails.

#### Acceptance Criteria

1. IF the CSV_File cannot be read due to a permissions error, THEN THE CLI SHALL exit with a non-zero exit code and print an error message to stderr indicating the file could not be accessed due to insufficient permissions
2. IF the CSV_File is empty (zero bytes), THEN THE CLI SHALL exit with a non-zero exit code and print an error message to stderr indicating the file is empty
3. THE CLI SHALL never write error messages to stdout
4. IF the CSV_File cannot be read due to an I/O error other than permissions or file-not-found, THEN THE CLI SHALL exit with a non-zero exit code and print an error message to stderr indicating the nature of the read failure
5. THE CLI SHALL include the file path in all error messages printed to stderr

### Requirement 6: Round-Trip Fidelity

**User Story:** As a developer, I want to verify that parsing and serialization are consistent, so that data integrity is maintained.

#### Acceptance Criteria

1. WHEN a valid CSV_File with a Header_Row and one or more Data_Rows is parsed to a JSON_Array and then converted back to CSV format, THE resulting CSV SHALL contain the same field values in the same row and column positions as the original
2. WHEN performing round-trip conversion, THE column order in the resulting CSV SHALL match the column order in the original Header_Row
3. WHEN performing round-trip conversion, differences in field quoting between the original and resulting CSV SHALL NOT be considered a fidelity failure provided the unquoted field values are identical
