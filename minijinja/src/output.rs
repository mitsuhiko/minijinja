use std::fmt;

use crate::utils::AutoEscape;
use crate::value::Value;

/// An abstraction over [`Write`](std::fmt::Write) for the rendering.
///
/// This is a utility type used in the engine which can be written into like one
/// can write into an [`std::fmt::Write`] value.  It's primarily used internally
/// in the engine but it's also passed to the custom formatter function.
///
/// Additionally it keeps track of the requested auto escape format of the
/// template so that the formatter can customize it's behavior based on the auto
/// escaping request.  The output itself however does not have functionality to
/// escape by itself.
pub struct Output<'a> {
    // Note on type design: this type partially exists so that in the future non
    // string outputs can be implemented.  Right now only the null writer exists
    // as alternative which is also infallible.  If writing to io::Write should
    // be added, then errors would need to be collected out of bounds as
    // fmt::Error has no support for carrying actual IO errors.
    w: &'a mut (dyn fmt::Write + 'a),
    capture_stack: Vec<String>,
    pub(crate) auto_escape: AutoEscape,
}

pub struct NullWriter;

impl<'a> Output<'a> {
    /// Creates an output writing to a string.
    pub(crate) fn with_string(buf: &'a mut String, auto_escape: AutoEscape) -> Self {
        Self {
            w: buf,
            capture_stack: Vec::new(),
            auto_escape,
        }
    }

    /// Creates a null output that writes nowhere.
    pub(crate) fn null() -> Self {
        static mut NULL_WRITER: NullWriter = NullWriter;
        Self {
            // SAFETY: this is safe as the null writer is a ZST
            w: unsafe { &mut NULL_WRITER },
            capture_stack: Vec::new(),
            auto_escape: AutoEscape::None,
        }
    }

    /// Begins capturing into a string.
    pub(crate) fn begin_capture(&mut self) {
        self.capture_stack.push(String::new());
    }

    /// Ends capturing and returns the captured string as value.
    pub(crate) fn end_capture(&mut self) -> Value {
        let captured = self.capture_stack.pop().unwrap();
        if !matches!(self.auto_escape, AutoEscape::None) {
            Value::from_safe_string(captured)
        } else {
            Value::from(captured)
        }
    }

    fn target(&mut self) -> &mut dyn fmt::Write {
        match self.capture_stack.last_mut() {
            Some(stream) => stream as _,
            None => self.w,
        }
    }

    /// Returns the current auto escape setting.
    #[inline]
    pub fn auto_escape(&self) -> AutoEscape {
        self.auto_escape
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
