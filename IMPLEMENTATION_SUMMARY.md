# Fix for MiniJinja Issue #803: Support for \x hex escape sequences

## Overview
Added support for hexadecimal escape sequences in the form `\xNN` where NN is a 2-digit hexadecimal number in MiniJinja template strings.

## Problem
MiniJinja previously supported these escape sequences:
- `\"`, `\\`, `/`, `\'` - literal characters
- `\b` → `\x08` (backspace)
- `\f` → `\x0C` (form feed)
- `\n` → newline
- `\r` → carriage return  
- `\t` → tab
- `\uNNNN` → Unicode escape sequences (4 hex digits)

But it did **not** support `\x` hex escape sequences like `\x00`, `\x41`, `\xff`, etc.

## Solution
Modified the `unescape` function in `minijinja/src/utils.rs` to add support for `\x` escape sequences.

### Changes Made

1. **Added new case in escape sequence matching** (line ~290 in `utils.rs`):
   ```rust
   'x' => {
       let val = ok!(self.parse_hex_byte(&mut char_iter));
       ok!(self.push_char(val as char));
   }
   ```

2. **Added new helper method** `parse_hex_byte`:
   ```rust
   fn parse_hex_byte(&self, chars: &mut Chars) -> Result<u8, Error> {
       let hexnum = chars.chain(repeat('\0')).take(2).collect::<String>();
       if hexnum.len() < 2 || hexnum.contains('\0') {
           return Err(ErrorKind::BadEscape.into());
       }
       u8::from_str_radix(&hexnum, 16).map_err(|_| ErrorKind::BadEscape.into())
   }
   ```

3. **Added comprehensive tests** in `utils.rs`:
   ```rust
   #[test]
   fn test_unescape_hex() {
       // Test basic hex escape sequences
       assert_eq!(unescape(r"\x41\x42\x43").unwrap(), "ABC");
       assert_eq!(unescape(r"\x00").unwrap(), "\0");
       assert_eq!(unescape(r"\x20").unwrap(), " ");
       assert_eq!(unescape(r"\xff").unwrap(), "\u{ff}");
       assert_eq!(unescape(r"\xFF").unwrap(), "\u{FF}");
       
       // Test invalid hex sequences should error
       assert!(unescape(r"\x").is_err());
       assert!(unescape(r"\x1").is_err());
       assert!(unescape(r"\xGG").is_err());
       assert!(unescape(r"\xZ1").is_err());
   }
   ```

4. **Created integration tests** to verify template functionality:
   - Basic hex escape sequences: `{{ "\x41\x42\x43" }}` → `"ABC"`
   - Null bytes: `{{ "\x00" }}` → `"\0"`
   - Mixed with other escapes: `{{ "Hello\x20World\x21" }}` → `"Hello World!"`
   - Error handling for invalid sequences

## Features

### Supported Formats
- **Lowercase hex**: `\xff` → character with value 255
- **Uppercase hex**: `\xFF` → character with value 255  
- **Leading zeros**: `\x00` → null character
- **Any valid 2-digit hex**: `\x41` → 'A' (ASCII 65)

### Error Handling
The implementation properly validates hex escape sequences and returns errors for:
- Incomplete sequences: `\x`, `\x1`
- Invalid hex digits: `\xGG`, `\xZ1`
- Missing digits after `\x`

### Integration
- Works seamlessly with existing escape sequences
- No breaking changes to existing functionality
- Maintains compatibility with JSON-like escape sequence behavior

## Testing
- ✅ All existing tests continue to pass
- ✅ New unit tests for `unescape` function
- ✅ Integration tests for template rendering
- ✅ Error handling tests
- ✅ Mixed escape sequence tests

## Example Usage

```jinja2
{{ "\x48\x65\x6c\x6c\x6f" }}        <!-- Outputs: Hello -->
{{ "\x41\x42\x43" }}                <!-- Outputs: ABC -->  
{{ "\x00" }}                        <!-- Outputs: null byte -->
{{ "Line1\nLine2\x20Tab:\t\x41" }}  <!-- Mixed escapes work -->
```

## Backwards Compatibility
This change is fully backwards compatible. All existing templates will continue to work exactly as before, and the new `\x` escape sequences are purely additive functionality.

The implementation follows the same error handling patterns as existing escape sequences, ensuring consistent behavior across the codebase.