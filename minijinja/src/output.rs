use std::{fmt, io};

use crate::utils::AutoEscape;
use crate::value::Value;

/// An abstraction over [`Write`](std::fmt::Write) for the rendering.
///
/// This is a utility type used in the engine which can be written into like one
/// can write into an [`std::fmt::Write`] value.  It's primarily used internally
/// in the engine but it's also passed to the custom formatter function.
pub struct Output<'a> {
    w: &'a mut (dyn fmt::Write + 'a),
    capture_stack: Vec<String>,
}

impl<'a> Output<'a> {
    /// Creates an output writing to a string.
    pub(crate) fn with_string(buf: &'a mut String) -> Self {
        Self {
            w: buf,
            capture_stack: Vec::new(),
        }
    }

    pub(crate) fn with_write(w: &'a mut (dyn fmt::Write + 'a)) -> Self {
        Self {
            w,
            capture_stack: Vec::new(),
        }
    }

    /// Creates a null output that writes nowhere.
    pub(crate) fn null() -> Self {
        static mut NULL_WRITER: NullWriter = NullWriter;
        Self {
            // SAFETY: this is safe as the null writer is a ZST
            w: unsafe { &mut NULL_WRITER },
            capture_stack: Vec::new(),
        }
    }

    /// Begins capturing into a string.
    pub(crate) fn begin_capture(&mut self) {
        self.capture_stack.push(String::new());
    }

    /// Ends capturing and returns the captured string as value.
    pub(crate) fn end_capture(&mut self, auto_escape: AutoEscape) -> Value {
        let captured = self.capture_stack.pop().unwrap();
        if !matches!(auto_escape, AutoEscape::None) {
            Value::from_safe_string(captured)
        } else {
            Value::from(captured)
        }
    }

    #[inline(always)]
    fn target(&mut self) -> &mut dyn fmt::Write {
        match self.capture_stack.last_mut() {
            Some(stream) => stream as _,
            None => self.w,
        }
    }

    /// Writes some data to the underlying buffer contained within this output.
    #[inline]
    pub fn write_str(&mut self, s: &str) -> fmt::Result {
        self.target().write_str(s)
    }

    /// Writes some formatted information into this instance.
    #[inline]
    pub fn write_fmt(&mut self, a: fmt::Arguments<'_>) -> fmt::Result {
        self.target().write_fmt(a)
    }
}

impl fmt::Write for Output<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        fmt::Write::write_str(self.target(), s)
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        fmt::Write::write_char(self.target(), c)
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        fmt::Write::write_fmt(self.target(), args)
    }
}

pub struct NullWriter;

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
