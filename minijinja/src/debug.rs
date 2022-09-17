use std::fmt;

use crate::compiler::tokens::Span;
use crate::error::ErrorKind;
use crate::value::Value;

/// This is a snapshot of the debug information.
#[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
#[derive(Default)]
pub(crate) struct DebugInfo {
    pub(crate) template_source: Option<String>,
    pub(crate) context: Option<Value>,
    pub(crate) referenced_names: Option<Vec<String>>,
}

struct VarPrinter<'x>(Value, &'x [String]);

impl<'x> fmt::Debug for VarPrinter<'x> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_struct("Referenced variables:");
        let mut vars = self.1.to_owned();
        vars.sort();
        for var in &vars {
            match self.0.get_attr(var) {
                Ok(val) => m.field(var, &val),
                Err(_) => m.field(var, &Value::UNDEFINED),
            };
        }
        m.finish()
    }
}

impl DebugInfo {
    /// If available this contains a reference to the source string.
    pub fn source(&self) -> Option<&str> {
        self.template_source.as_deref()
    }

    /// Provides access to a snapshot of the context.
    ///
    /// The context is created at the time the error was created if that error
    /// happened during template rendering.
    pub fn context(&self) -> Option<Value> {
        self.context.clone()
    }

    /// Returns a narrowed down set of referenced names from the context
    /// where the error happened.
    ///
    /// This function is currently internal and only used for the default
    /// error printing.  This could be exposed but it's a highly specific
    /// API.
    pub(crate) fn referenced_names(&self) -> Option<&[String]> {
        self.referenced_names.as_deref()
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
    if let Some(source) = info.source() {
        let title = format!(
            " {} ",
            name.unwrap_or_default()
                .rsplit(&['/', '\\'])
                .next()
                .unwrap_or("Template Source")
        );
        writeln!(f)?;
        writeln!(f, "{:-^1$}", title, 79).unwrap();
        let lines: Vec<_> = source.lines().enumerate().collect();
        let idx = line.unwrap_or(1).saturating_sub(1);
        let skip = idx.saturating_sub(3);
        let pre = lines.iter().skip(skip).take(3.min(idx)).collect::<Vec<_>>();
        let post = lines.iter().skip(idx + 1).take(3).collect::<Vec<_>>();
        for (idx, line) in pre {
            writeln!(f, "{:>4} | {}", idx + 1, line).unwrap();
        }

        writeln!(f, "{:>4} > {}", idx + 1, lines[idx].1).unwrap();
        if let Some(span) = span {
            if span.start_line == span.end_line {
                writeln!(
                    f,
                    "     i {}{} {}",
                    " ".repeat(span.start_col),
                    "^".repeat(span.end_col - span.start_col),
                    kind,
                )?;
            }
        }

        for (idx, line) in post {
            writeln!(f, "{:>4} | {}", idx + 1, line).unwrap();
        }
        write!(f, "{:~^1$}", "", 79).unwrap();
    }
    if let Some(ctx) = info.context() {
        if let Some(vars) = info.referenced_names() {
            writeln!(f)?;
            writeln!(f, "{:#?}", VarPrinter(ctx, vars))?;
        }
        write!(f, "{:-^1$}", "", 79).unwrap();
    }
    Ok(())
}
