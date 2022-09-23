#[cfg(feature = "internal_debug")]
use std::fmt;

#[cfg(test)]
use similar_asserts::assert_eq;

use crate::compiler::tokens::Span;
use crate::value::Value;

/// This loop has the loop var.
pub const LOOP_FLAG_WITH_LOOP_VAR: u8 = 1;

/// This loop is recursive.
pub const LOOP_FLAG_RECURSIVE: u8 = 2;

/// Rust type to represent locals.
pub type LocalId = u8;

/// The maximum number of filters/tests that can be cached.
pub const MAX_LOCALS: usize = 50;

/// Represents an instruction for the VM.
#[cfg_attr(feature = "internal_debug", derive(Debug))]
#[derive(Clone)]
pub enum Instruction<'source> {
    /// Emits raw source
    EmitRaw(&'source str),

    /// Stores a variable (only possible in for loops)
    StoreLocal(&'source str),

    /// Load a variable,
    Lookup(&'source str),

    /// Looks up an attribute.
    GetAttr(&'source str),

    /// Looks up an item.
    GetItem,

    /// Performs a slice operation.
    Slice,

    /// Loads a constant value.
    LoadConst(Value),

    /// Builds a map of the last n pairs on the stack.
    BuildMap(usize),

    /// Builds a kwargs map of the last n pairs on the stack.
    BuildKwargs(usize),

    /// Builds a list of the last n pairs on the stack.
    BuildList(usize),

    /// Unpacks a list into N stack items.
    UnpackList(usize),

    /// Appends to the list.
    ListAppend,

    /// Add the top two values
    Add,

    /// Subtract the top two values
    Sub,

    /// Multiply the top two values
    Mul,

    /// Divide the top two values
    Div,

    /// Integer divde the top two values as "integer".
    ///
    /// Note that in MiniJinja this currently uses an euclidean
    /// division to match the rem implementation.  In Python this
    /// instead uses a flooring division and a flooring remainder.
    IntDiv,

    /// Calculate the remainder the top two values
    Rem,

    /// x to the power of y.
    Pow,

    /// Negates the value.
    Neg,

    /// `=` operator
    Eq,

    /// `!=` operator
    Ne,

    /// `>` operator
    Gt,

    /// `>=` operator
    Gte,

    /// `<` operator
    Lt,

    /// `<=` operator
    Lte,

    /// Unary not
    Not,

    /// String concatenation operator
    StringConcat,

    /// Performs a containment check
    In,

    /// Apply a filter.
    ApplyFilter(&'source str, usize, LocalId),

    /// Perform a filter.
    PerformTest(&'source str, usize, LocalId),

    /// Emit the stack top as output
    Emit,

    /// Starts a loop
    ///
    /// The argument are loop flags.
    PushLoop(u8),

    /// Starts a with block.
    PushWith,

    /// Does a single loop iteration
    ///
    /// The argument is the jump target for when the loop
    /// ends and must point to a `PopFrame` instruction.
    Iterate(usize),

    /// Pops the topmost frame
    PopFrame,

    /// True if the value is undefined
    IsUndefined,

    /// Jump to a specific instruction
    Jump(usize),

    /// Jump if the stack top evaluates to false
    JumpIfFalse(usize),

    /// Jump if the stack top evaluates to false or pops the value
    JumpIfFalseOrPop(usize),

    /// Jump if the stack top evaluates to true or pops the value
    JumpIfTrueOrPop(usize),

    /// Call into a block.
    CallBlock(&'source str),

    /// Loads block from a template with name on stack ("extends")
    LoadBlocks,

    /// Includes another template.
    Include(bool),

    /// Sets the auto escape flag to the current value.
    PushAutoEscape,

    /// Resets the auto escape flag to the previous value.
    PopAutoEscape,

    /// Begins capturing of output.
    BeginCapture,

    /// Ends capturing of output.
    EndCapture,

    /// Calls a global function
    CallFunction(&'source str, usize),

    /// Calls a method
    CallMethod(&'source str, usize),

    /// Calls an object
    CallObject(usize),

    /// Duplicates the top item
    DupTop,

    /// Discards the top item
    DiscardTop,

    /// A fast super instruction without intermediate capturing.
    FastSuper,

    /// A fast loop recurse instruction without intermediate capturing.
    FastRecurse,

    /// Builds a macro on the stack.
    BuildMacro(&'source str, usize),

    /// Builds a module
    ExportLocals,

    /// Breaks from the interpreter loop (exists a function)
    Return,
}

#[derive(Copy, Clone)]
struct LineInfo {
    first_instruction: u32,
    line: u32,
}

#[cfg(feature = "debug")]
#[derive(Copy, Clone)]
struct SpanInfo {
    first_instruction: u32,
    span: Option<Span>,
}

/// Wrapper around instructions to help with location management.
pub struct Instructions<'source> {
    pub(crate) instructions: Vec<Instruction<'source>>,
    line_infos: Vec<LineInfo>,
    #[cfg(feature = "debug")]
    span_infos: Vec<SpanInfo>,
    name: &'source str,
    source: &'source str,
}

impl<'source> Instructions<'source> {
    /// Creates a new instructions object.
    pub fn new(name: &'source str, source: &'source str) -> Instructions<'source> {
        Instructions {
            instructions: Vec::new(),
            line_infos: Vec::new(),
            #[cfg(feature = "debug")]
            span_infos: Vec::new(),
            name,
            source,
        }
    }

    /// Returns the name of the template.
    pub fn name(&self) -> &'source str {
        self.name
    }

    /// Returns the source reference.
    pub fn source(&self) -> &'source str {
        self.source
    }

    /// Returns an instruction by index
    #[inline(always)]
    pub fn get(&self, idx: usize) -> Option<&Instruction<'source>> {
        self.instructions.get(idx)
    }

    /// Returns an instruction by index mutably
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Instruction<'source>> {
        self.instructions.get_mut(idx)
    }

    /// Adds a new instruction
    pub fn add(&mut self, instr: Instruction<'source>) -> usize {
        let rv = self.instructions.len();
        self.instructions.push(instr);
        rv
    }

    fn add_line_record(&mut self, instr: usize, line: usize) {
        let same_loc = self
            .line_infos
            .last()
            .map_or(false, |last_loc| last_loc.line as usize == line);
        if !same_loc {
            self.line_infos.push(LineInfo {
                first_instruction: instr as u32,
                line: line as u32,
            });
        }
    }

    /// Adds a new instruction with line number.
    pub fn add_with_line(&mut self, instr: Instruction<'source>, line: usize) -> usize {
        let rv = self.add(instr);
        self.add_line_record(rv, line);

        // if we follow up to a valid span with no more span, clear it out
        #[cfg(feature = "debug")]
        {
            if self.span_infos.last().map_or(false, |x| x.span.is_some()) {
                self.span_infos.push(SpanInfo {
                    first_instruction: rv as u32,
                    span: None,
                });
            }
        }
        rv
    }

    /// Adds a new instruction with span.
    pub fn add_with_span(&mut self, instr: Instruction<'source>, span: Span) -> usize {
        let rv = self.add(instr);
        #[cfg(feature = "debug")]
        {
            let same_loc = self
                .span_infos
                .last()
                .map_or(false, |last_loc| last_loc.span == Some(span));
            if !same_loc {
                self.span_infos.push(SpanInfo {
                    first_instruction: rv as u32,
                    span: Some(span),
                });
            }
        }
        self.add_line_record(rv, span.start_line);
        rv
    }

    /// Looks up the line for an instruction
    pub fn get_line(&self, idx: usize) -> Option<usize> {
        let loc = match self
            .line_infos
            .binary_search_by_key(&idx, |x| x.first_instruction as usize)
        {
            Ok(idx) => &self.line_infos[idx as usize],
            Err(0) => return None,
            Err(idx) => &self.line_infos[idx as usize - 1],
        };
        Some(loc.line as usize)
    }

    /// Looks up a span for an instruction.
    pub fn get_span(&self, idx: usize) -> Option<Span> {
        #[cfg(feature = "debug")]
        {
            let loc = match self
                .span_infos
                .binary_search_by_key(&idx, |x| x.first_instruction as usize)
            {
                Ok(idx) => &self.span_infos[idx as usize],
                Err(0) => return None,
                Err(idx) => &self.span_infos[idx as usize - 1],
            };
            loc.span
        }
        #[cfg(not(feature = "debug"))]
        {
            let _ = idx;
            None
        }
    }

    /// Returns a list of all names referenced in the current block backwards
    /// from the given pc.
    #[cfg(feature = "debug")]
    pub fn get_referenced_names(&self, idx: usize) -> Vec<&'source str> {
        let mut rv = Vec::new();
        let idx = idx.min(self.instructions.len() - 1);
        for instr in self.instructions[..=idx].iter().rev() {
            let name = match instr {
                Instruction::Lookup(name)
                | Instruction::StoreLocal(name)
                | Instruction::CallFunction(name, _) => *name,
                Instruction::PushLoop(flags) if flags & LOOP_FLAG_WITH_LOOP_VAR != 0 => "loop",
                Instruction::PushLoop(_) | Instruction::PushWith => break,
                _ => continue,
            };
            if !rv.contains(&name) {
                rv.push(name);
            }
        }
        rv
    }

    /// Returns the number of instructions
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Do we have any instructions?
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }
}

#[cfg(feature = "internal_debug")]
impl<'source> fmt::Debug for Instructions<'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct InstructionWrapper<'a>(usize, &'a Instruction<'a>, Option<usize>);

        impl<'a> fmt::Debug for InstructionWrapper<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:>05x} | {:?}", self.0, self.1,)?;
                if let Some(line) = self.2 {
                    write!(f, "  [line {}]", line)?;
                }
                Ok(())
            }
        }

        let mut list = f.debug_list();
        let mut last_line = None;
        for (idx, instr) in self.instructions.iter().enumerate() {
            let line = self.get_line(idx);
            list.entry(&InstructionWrapper(
                idx,
                instr,
                if line != last_line { line } else { None },
            ));
            last_line = line;
        }
        list.finish()
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn test_sizes() {
    assert_eq!(std::mem::size_of::<Instruction>(), 32);
}
