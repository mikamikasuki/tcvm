use thiserror::Error;

use crate::compiler::CompileError;
use crate::lua::stash::StashedError;

#[derive(Debug, Error)]
pub enum LoadError {
    /// The rendered syntax error reports.
    #[error("{0}")]
    Parse(String),
    /// `chunk` is the chunk id the message is prefixed with, as in Lua.
    #[error("{chunk}:{}: {}", .error.line_number, .error.kind)]
    Compile { chunk: String, error: CompileError },
    /// The file couldn't be opened or read.
    #[error("{0}")]
    File(String),
    /// Not loadable text: a binary chunk, not UTF-8, or excluded by the mode.
    #[error("{0}")]
    Chunk(String),
    #[error("internal: {0}")]
    Internal(&'static str),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("bad executor mode")]
    BadMode,
    /// The main thread yielded to the host before completing. `finish`/
    /// `execute` can't surface yielded values across their `'gc`
    /// boundary, so they raise this rather than reporting bogus
    /// completion. To consume the yielded values and feed resume args
    /// back, drive the executor manually with `Executor::step` /
    /// `Lua::resume` instead.
    #[error("main thread yielded; use Lua::resume to continue")]
    MainYielded,
    /// User-thrown Lua error (`error(value)`); payload is the stashed value.
    /// Inspect / display via `Lua::enter` + `Fetchable::fetch`.
    #[error("lua error")]
    Lua(StashedError),
    #[error(transparent)]
    Type(#[from] TypeError),
}

#[derive(Debug, Error)]
pub enum TypeError {
    #[error("expected {expected}, got {got}")]
    Mismatch {
        expected: &'static str,
        got: &'static str,
    },
    #[error("expected {expected} value(s), got {got}")]
    Arity { expected: usize, got: usize },
}
