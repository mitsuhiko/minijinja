use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::{self, Write};

use crate::compiler::tokens::Span;
use crate::error::ErrorKind;
use crate::value::Value;

/// Maximum number of items shown per sequence or map in debug info.
pub(crate) const MAX_ITEMS: usize = 10;

/// Maximum nesting depth shown in debug info.
const MAX_DEPTH: usize = 4;

/// Maximum number of characters shown per string in debug info.
pub(crate) const MAX_STRING_CHARS: usize = 100;

thread_local! {
    // The current nesting depth while debug printing values with size
    // limits, or `None` if values are printed without limits.
    static LIMITED_DEPTH: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Restores the previous limited depth on drop.
struct LimitedDepthGuard(Option<usize>);

impl Drop for LimitedDepthGuard {
    fn drop(&mut self) {
        LIMITED_DEPTH.with(|x| x.set(self.0));
    }
}

fn with_limited_depth<R>(depth: Option<usize>, f: impl FnOnce() -> R) -> R {
    let _guard = LimitedDepthGuard(LIMITED_DEPTH.with(|x| x.replace(depth)));
    f()
}

/// Returns `true` if values are currently debug printed with size limits.
///
/// Size limits are enabled while debug info is rendered so that large
/// values (eg: the entire environment) do not drown out the relevant
/// information.  They are honored by the default object rendering and
/// string formatting, objects with custom rendering are not affected.
pub(crate) fn is_limited() -> bool {
    LIMITED_DEPTH.with(|x| x.get()).is_some()
}

/// Debug prints the items of a container with size limits.
///
/// If size limits are not enabled, this just invokes `f` which is expected
/// to print the items.  Otherwise, past the maximum depth `placeholder` is
/// written instead and `f` is invoked with the depth increased.
pub(crate) fn limited_container(
    f: &mut fmt::Formatter<'_>,
    placeholder: &str,
    items: impl FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
) -> fmt::Result {
    match LIMITED_DEPTH.with(|x| x.get()) {
        None => items(f),
        Some(depth) if depth >= MAX_DEPTH => f.write_str(placeholder),
        Some(depth) => with_limited_depth(Some(depth + 1), || items(f)),
    }
}

/// This is a snapshot of the debug information.
#[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
#[derive(Default)]
pub(crate) struct DebugInfo {
    pub(crate) template_source: Option<String>,
    pub(crate) referenced_locals: BTreeMap<String, Value>,
}

struct VarPrinter<'x>(&'x BTreeMap<String, Value>);

impl fmt::Debug for VarPrinter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("No referenced variables");
        }
        with_limited_depth(Some(0), || {
            let mut m = f.debug_struct("Referenced variables:");
            for (key, value) in self.0.iter() {
                m.field(key, value);
            }
            m.finish()
        })
    }
}

impl DebugInfo {
    pub fn source(&self) -> Option<&str> {
        self.template_source.as_deref()
    }
}

pub(super) fn render_debug_info(
    f: &mut fmt::Formatter,
    name: Option<&str>,
    kind: ErrorKind,
    line: Option<usize>,
    span: Option<Span>,
    info: &DebugInfo,
) -> fmt::Result {
    // without a line there is nothing in the source to point to
    if let (Some(source), Some(line)) = (info.source(), line) {
        let title = format!(
            " {} ",
            name.unwrap_or_default()
                .rsplit(&['/', '\\'])
                .next()
                .unwrap_or("Template Source")
        );
        ok!(writeln!(f));
        ok!(writeln!(f, "{:-^1$}", title, 79));
        ok!(render_source_excerpt(f, source, line, span, kind));
        ok!(write!(f, "{:~^1$}", "", 79));
    }
    ok!(writeln!(f));
    ok!(writeln!(f, "{:#?}", VarPrinter(&info.referenced_locals)));
    write!(f, "{:-^1$}", "", 79)
}

/// Renders the lines around the given (1-indexed) line.
///
/// The span is marked if it starts on that line.
fn render_source_excerpt(
    f: &mut fmt::Formatter,
    source: &str,
    line: usize,
    span: Option<Span>,
    kind: ErrorKind,
) -> fmt::Result {
    const CONTEXT_LINES: usize = 3;

    let idx = line.saturating_sub(1);
    let first = idx.saturating_sub(CONTEXT_LINES);
    let mut lines = source.lines().enumerate().skip(first);

    for (lineno, text) in lines.by_ref().take(idx - first + 1) {
        if lineno != idx {
            ok!(writeln!(f, "{:>4} | {}", lineno + 1, text));
            continue;
        }
        ok!(writeln!(f, "{:>4} > {}", lineno + 1, text));
        if let Some(span) = span.filter(|x| x.start_line as usize == line) {
            ok!(render_span_marker(f, text, span, kind));
        }
    }

    for (lineno, text) in lines.take(CONTEXT_LINES) {
        ok!(writeln!(f, "{:>4} | {}", lineno + 1, text));
    }
    Ok(())
}

/// Underlines the span on its first line.
///
/// Spans that continue on later lines are underlined to the end of the
/// line.  Tabs before the span are retained so that the marker lines up
/// with the source line.
fn render_span_marker(
    f: &mut fmt::Formatter,
    text: &str,
    span: Span,
    kind: ErrorKind,
) -> fmt::Result {
    let start = span.start_col as usize;
    let end = if span.end_line == span.start_line {
        span.end_col as usize
    } else {
        text.chars().count()
    };
    ok!(f.write_str("     i "));
    let mut chars = text.chars();
    for _ in 0..start {
        let c = chars.next().unwrap_or(' ');
        ok!(f.write_char(if c == '\t' { '\t' } else { ' ' }));
    }
    for _ in 0..end.saturating_sub(start).max(1) {
        ok!(f.write_char('^'));
    }
    writeln!(f, " {kind}")
}
