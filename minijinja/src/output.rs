#[cfg(feature = "debug")]
use std::ops::Range;
use std::ptr::addr_of_mut;
#[cfg(feature = "debug")]
use std::ptr::NonNull;
#[cfg(feature = "debug")]
use std::sync::Arc;
use std::{fmt, io};

#[cfg(feature = "debug")]
use crate::compiler::instructions::Instructions;
#[cfg(feature = "debug")]
use crate::compiler::tokens::Span;

use crate::error::{Error, ErrorKind};
use crate::utils::AutoEscape;
use crate::value::Value;

/// How should output be captured?
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "unstable_machinery_serde", derive(serde::Serialize))]
pub enum CaptureMode {
    Capture,
    #[cfg(feature = "multi_template")]
    Discard,
}

/// An abstraction over [`fmt::Write`](std::fmt::Write) for the rendering.
///
/// This is a utility type used in the engine which can be written into like one
/// can write into an [`std::fmt::Write`] value.  It's primarily used internally
/// in the engine but it's also passed to the custom formatter function.
pub struct Output<'a> {
    w: *mut (dyn fmt::Write + 'a),
    target: *mut (dyn fmt::Write + 'a),
    capture_stack: Vec<Option<String>>,
    // bytes written that were not yet charged against the fuel.
    #[cfg(feature = "fuel")]
    uncharged_bytes: usize,
    #[cfg(feature = "debug")]
    span_recorder: Option<NonNull<OutputSpanRecorder>>,
}

impl<'a> Output<'a> {
    /// Creates a new output.
    pub(crate) fn new(w: &'a mut (dyn fmt::Write + 'a)) -> Self {
        Self {
            w,
            target: w,
            capture_stack: Vec::new(),
            #[cfg(feature = "fuel")]
            uncharged_bytes: 0,
            #[cfg(feature = "debug")]
            span_recorder: None,
        }
    }

    /// Creates a new output that writes into a span recorder.
    #[cfg(feature = "debug")]
    pub(crate) fn with_span_recorder(recorder: &'a mut OutputSpanRecorder) -> Self {
        // both the writer and the recorder pointer are derived from the same
        // raw pointer.  They are never dereferenced at the same time.
        let ptr: *mut OutputSpanRecorder = recorder;
        Self {
            w: ptr,
            target: ptr,
            capture_stack: Vec::new(),
            span_recorder: NonNull::new(ptr),
            #[cfg(feature = "fuel")]
            uncharged_bytes: 0,
        }
    }

    /// Creates a null output that writes nowhere.
    pub(crate) fn null() -> Self {
        // The null writer also has a single entry on the discarding capture
        // stack.  In fact, `w` is more or less useless here as we always
        // shadow it.  This is done so that `is_discarding` returns true.
        Self {
            w: NullWriter::get_mut(),
            target: NullWriter::get_mut(),
            capture_stack: vec![None],
            #[cfg(feature = "fuel")]
            uncharged_bytes: 0,
            #[cfg(feature = "debug")]
            span_recorder: None,
        }
    }

    /// Begins capturing into a string or discard.
    pub(crate) fn begin_capture(&mut self, mode: CaptureMode) {
        self.capture_stack.push(match mode {
            CaptureMode::Capture => Some(String::new()),
            #[cfg(feature = "multi_template")]
            CaptureMode::Discard => None,
        });
        self.retarget();
    }

    /// Ends capturing and returns the captured string as value.
    pub(crate) fn end_capture(&mut self, auto_escape: &AutoEscape) -> Value {
        let rv = if let Some(captured) = self.capture_stack.pop().unwrap() {
            if !matches!(auto_escape, AutoEscape::None) {
                Value::from_safe_string(captured)
            } else {
                Value::from(captured)
            }
        } else {
            Value::UNDEFINED
        };
        self.retarget();
        rv
    }

    fn retarget(&mut self) {
        self.target = match self.capture_stack.last_mut() {
            Some(Some(stream)) => stream,
            Some(None) => NullWriter::get_mut(),
            None => self.w,
        };
    }

    #[inline(always)]
    fn target(&mut self) -> &mut dyn fmt::Write {
        // SAFETY: this is safe because we carefully maintain the capture stack
        // to update self.target whenever it's modified
        unsafe { &mut *self.target }
    }

    /// Returns `true` if the output is discarding.
    #[cfg(feature = "multi_template")]
    #[inline(always)]
    pub(crate) fn is_discarding(&self) -> bool {
        matches!(self.capture_stack.last(), Some(None))
    }

    /// Returns the number of bytes written that were not charged yet.
    #[cfg(feature = "fuel")]
    #[inline(always)]
    pub(crate) fn uncharged_bytes(&self) -> usize {
        self.uncharged_bytes
    }

    /// Returns the number of bytes written since the last call and resets
    /// the counter.
    #[cfg(feature = "fuel")]
    #[inline(always)]
    pub(crate) fn take_uncharged_bytes(&mut self) -> usize {
        std::mem::take(&mut self.uncharged_bytes)
    }

    /// If output spans are recorded and the output is not captured, this
    /// returns the current position in the final output.
    #[cfg(feature = "debug")]
    #[inline(always)]
    pub(crate) fn span_start(&self) -> Option<usize> {
        match self.span_recorder {
            // SAFETY: the recorder outlives the output and is not otherwise
            // borrowed while we look at it.
            Some(recorder) if self.capture_stack.is_empty() => {
                Some(unsafe { recorder.as_ref() }.buf.len())
            }
            _ => None,
        }
    }

    /// Records the output written since `start` (as returned by
    /// [`span_start`](Self::span_start)) as coming from the given instruction.
    #[cfg(feature = "debug")]
    pub(crate) fn record_span(
        &mut self,
        start: usize,
        instructions: &Instructions<'_>,
        pc: u32,
        kind: OutputSpanKind,
    ) {
        if let Some(mut recorder) = self.span_recorder {
            // SAFETY: the recorder outlives the output and is not otherwise
            // borrowed while we modify it.
            unsafe { recorder.as_mut() }.record(start, instructions, pc, kind);
        }
    }

    /// Writes some data to the underlying buffer contained within this output.
    #[inline]
    pub fn write_str(&mut self, s: &str) -> fmt::Result {
        fmt::Write::write_str(self, s)
    }

    /// Writes some formatted information into this instance.
    #[inline]
    pub fn write_fmt(&mut self, a: fmt::Arguments<'_>) -> fmt::Result {
        fmt::Write::write_fmt(self, a)
    }
}

impl fmt::Write for Output<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        #[cfg(feature = "fuel")]
        {
            self.uncharged_bytes = self.uncharged_bytes.saturating_add(s.len());
        }
        fmt::Write::write_str(self.target(), s)
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        #[cfg(feature = "fuel")]
        {
            self.uncharged_bytes = self.uncharged_bytes.saturating_add(c.len_utf8());
        }
        fmt::Write::write_char(self.target(), c)
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        // with fuel tracking all writes need to go through `write_str` so
        // that they are counted.
        #[cfg(feature = "fuel")]
        {
            fmt::write(self, args)
        }
        #[cfg(not(feature = "fuel"))]
        {
            fmt::Write::write_fmt(self.target(), args)
        }
    }
}

pub struct NullWriter;

impl NullWriter {
    /// Returns a reference to the null writer.
    pub fn get_mut() -> &'static mut NullWriter {
        static mut NULL_WRITER: NullWriter = NullWriter;
        // SAFETY: this is safe as the null writer is a ZST
        unsafe { &mut *addr_of_mut!(NULL_WRITER) }
    }
}

impl fmt::Write for NullWriter {
    #[inline]
    fn write_str(&mut self, _s: &str) -> fmt::Result {
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, _c: char) -> fmt::Result {
        Ok(())
    }
}

pub struct WriteWrapper<W> {
    pub w: W,
    pub err: Option<io::Error>,
}

impl<W> WriteWrapper<W> {
    /// Replaces the given error with the held error if available.
    pub fn take_err(&mut self, original: Error) -> Error {
        self.err
            .take()
            .map(|io_err| {
                Error::new(ErrorKind::WriteFailure, "I/O error during rendering")
                    .with_source(io_err)
            })
            .unwrap_or(original)
    }
}

impl<W: io::Write> fmt::Write for WriteWrapper<W> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.w.write_all(s.as_bytes()).map_err(|e| {
            self.err = Some(e);
            fmt::Error
        })
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.w
            .write_all(c.encode_utf8(&mut [0; 4]).as_bytes())
            .map_err(|e| {
                self.err = Some(e);
                fmt::Error
            })
    }
}

/// Describes what produced an [`OutputSpan`].
#[cfg(feature = "debug")]
#[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OutputSpanKind {
    /// Template data that was written verbatim.
    ///
    /// The output of such a span is identical to the source it points to,
    /// so offsets within the span map one-to-one to the template source.
    Raw,
    /// The output of an expression.
    ///
    /// This is used for `{{ ... }}` but also for the output of
    /// `{% filter %}` and `{% call %}` blocks, in which case the span
    /// points to the filter and call expression respectively.
    Expression,
}

/// Associates a range of rendered output with the template source that
/// produced it.
///
/// Output spans are returned by
/// [`Template::render_with_spans`](crate::Template::render_with_spans).
/// Spans are only recorded for output that is directly written to the
/// final output.  Output that is captured first (such as the output of
/// macros, `{% filter %}` or `{% call %}` blocks) is attributed to the
/// expression that emitted the captured value.  Output of included
/// templates, blocks and `super()` calls is attributed to the template
/// that contains the source that was rendered.
#[cfg(feature = "debug")]
#[cfg_attr(docsrs, doc(cfg(feature = "debug")))]
#[derive(Clone, PartialEq, Eq)]
pub struct OutputSpan {
    name: Arc<str>,
    kind: OutputSpanKind,
    output_start: usize,
    output_end: usize,
    span: Span,
}

#[cfg(feature = "debug")]
impl fmt::Debug for OutputSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputSpan")
            .field("name", &self.name())
            .field("kind", &self.kind())
            .field("output_range", &self.output_range())
            .field("source_range", &self.source_range())
            .field("line", &self.line())
            .finish()
    }
}

#[cfg(feature = "debug")]
impl OutputSpan {
    /// Returns the name of the template that produced the output.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns what kind of source produced the output.
    pub fn kind(&self) -> OutputSpanKind {
        self.kind
    }

    /// Returns the byte range in the rendered output.
    pub fn output_range(&self) -> Range<usize> {
        self.output_start..self.output_end
    }

    /// Returns the byte range in the source of the template.
    ///
    /// For [`Raw`](OutputSpanKind::Raw) spans this is exactly the source
    /// that was written.  For [`Expression`](OutputSpanKind::Expression)
    /// spans this is the source of the expression (without delimiters).
    pub fn source_range(&self) -> Range<usize> {
        self.span.start_offset as usize..self.span.end_offset as usize
    }

    /// Returns the line (1-indexed) in the template source where the span starts.
    pub fn line(&self) -> usize {
        self.span.start_line as usize
    }
}

/// Collects output together with the spans that produced it.
#[cfg(feature = "debug")]
#[derive(Default)]
pub(crate) struct OutputSpanRecorder {
    buf: String,
    spans: Vec<OutputSpan>,
    last_name: Option<Arc<str>>,
}

#[cfg(feature = "debug")]
impl OutputSpanRecorder {
    /// Creates a new recorder with a given capacity for the output.
    pub fn with_capacity(capacity: usize) -> OutputSpanRecorder {
        OutputSpanRecorder {
            buf: String::with_capacity(capacity),
            ..Default::default()
        }
    }

    fn record(
        &mut self,
        start: usize,
        instructions: &Instructions<'_>,
        pc: u32,
        kind: OutputSpanKind,
    ) {
        let end = self.buf.len();
        if start == end {
            return;
        }
        // the code generator attaches spans to all emitting instructions,
        // fall back to the line just in case.
        let span = instructions.get_span(pc).unwrap_or_else(|| {
            let line = instructions.get_line(pc).unwrap_or(0) as u16;
            Span {
                start_line: line,
                end_line: line,
                ..Span::default()
            }
        });
        let name = match self.last_name {
            Some(ref last) if &**last == instructions.name() => last.clone(),
            _ => {
                let name: Arc<str> = Arc::from(instructions.name());
                self.last_name = Some(name.clone());
                name
            }
        };
        self.spans.push(OutputSpan {
            name,
            kind,
            output_start: start,
            output_end: end,
            span,
        });
    }

    /// Returns the output and the recorded spans.
    pub fn into_parts(self) -> (String, Vec<OutputSpan>) {
        (self.buf, self.spans)
    }
}

#[cfg(feature = "debug")]
impl fmt::Write for OutputSpanRecorder {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.buf.push_str(s);
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.buf.push(c);
        Ok(())
    }
}
