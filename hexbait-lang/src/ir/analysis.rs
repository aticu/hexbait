//! Performs static analysis on the IR to ensure that the input is well formed.

use super::File;

/// The names resolved for each spanned symbol.
// TODO: implement this with fields
pub struct ResolvedNames {}

/// The error returned upon a failed analysis.
#[derive(Debug)]
pub struct AnalysisError {}

/// Checks if the file is well formed.
// TODO: add an error type here
pub fn check_ir(_file: &File) -> Result<ResolvedNames, AnalysisError> {
    // TODO: check types
    // TODO: resolve names
    // TODO: ensure that endianness is properly specified before parsing fields
    // TODO: ensure no errors are contained
    // TODO: ensure alignment is a power of two
    // TODO: ensure that alignment fits into u64
    // TODO: ensure that integers are non-zero length
    // TODO: ensure that non-byte-aligned integers are only allowed in bitfields
    // TODO: ensure that all field accesses are valid (both field access and in current struct)
    // TODO: ensure comparison operations are well types (== and != for all, but others only for ints)
    // TODO: ensure assertion and warning messages are utf8
    // TODO: ensure that $last is only used if $len > 0
    // TODO: ensure that $parent, $last and $len are only used in correct contexts
    // TODO: ensure sensible behavior about struct nested in scopes and if declarations
    Ok(ResolvedNames {})
}
