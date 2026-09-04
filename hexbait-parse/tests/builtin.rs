//! Implements a test that ensures that the built-in types build without error.

/// Check that the built-ins work.
#[test]
fn builtins_work() {
    hexbait_builtin_parsers::built_in_format_descriptions();
}
