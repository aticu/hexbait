//! Performs static analysis on the IR to ensure that the input is well formed.

use super::File;

/// The names resolved for each spanned symbol.
// TODO: implement this with fields
pub struct ResolvedNames {}

/// Checks if the file is well formed.
// TODO: add an error type here
pub fn check(file: &File) -> Result<ResolvedNames, ()> {
    // TODO: check types
    // TODO: resolve names
    // TODO: ensure that endianness is properly specified before parsing fields
    // TODO: ensure no errors are contained
    // TODO: ensure alignment is a power of two
    // TODO: ensure that alignment fits into u64
    // TODO: ensure that integers are non-zero length
    // TODO: ensure that non-byte-aligned integers are only allowed in bitfields
    Ok(ResolvedNames {})
}
