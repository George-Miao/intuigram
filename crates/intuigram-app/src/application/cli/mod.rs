//! Command-line definition and conversion into typed application arguments.

mod convert;
mod definition;

#[cfg(test)]
pub(super) use convert::help_text;
pub(super) use convert::{parse_arguments, print_help};
