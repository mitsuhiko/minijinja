#[cfg(feature = "internal_debug")]
use std::fmt;

use crate::value::Value;

/// This loop has the loop var.
pub const LOOP_FLAG_WITH_LOOP_VAR: u8 = 1;

/// This loop is recursive.
pub const LOOP_FLAG_RECURSIVE: u8 = 2;

/// Represents an instruction for the VM.
#[derive(Clone, PartialEq, Eq)]
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

    /// Loads a constant value.
    LoadConst(Value),

    /// Builds a map of the last n pairs on the stack.
    BuildMap(usize),

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
    ApplyFilter(&'source str),

    /// Perform a filter.
    PerformTest(&'source str),

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
    CallFunction(&'source str),

    /// Calls a method
    CallMethod(&'source str),

    /// Calls an object
    CallObject,

    /// Duplicates the top item
    DupTop,

    /// Discards the top item
    DiscardTop,

    /// A fast super instruction without intermediate capturing.
    FastSuper,

    /// A fast loop recurse instruction without intermediate capturing.
    FastRecurse,

    /// A nop
    #[allow(unused)]
    Nop,
}

#[cfg(feature = "internal_debug")]
impl<'source> fmt::Debug for Instruction<'source> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Instruction::EmitRaw(s) => write!(f, "EMIT_RAW (string {:?})", s),
            Instruction::StoreLocal(n) => write!(f, "STORE_LOCAL (var {:?})", n),
            Instruction::Lookup(n) => write!(f, "LOOKUP (var {:?})", n),
            Instruction::GetAttr(n) => write!(f, "GETATTR (key {:?})", n),
            Instruction::GetItem => write!(f, "GETITEM"),
            Instruction::LoadConst(ref v) => write!(f, "LOAD_CONST (value {:?})", v),
            Instruction::BuildMap(n) => write!(f, "BUILD_MAP ({:?} pairs)", n),
            Instruction::BuildList(n) => write!(f, "BUILD_LIST ({:?} items)", n),
            Instruction::UnpackList(n) => write!(f, "UNPACK_LIST ({:?} items)", n),
            Instruction::ListAppend => write!(f, "LIST_APPEND"),
            Instruction::Add => write!(f, "ADD"),
            Instruction::Sub => write!(f, "SUB"),
            Instruction::Mul => write!(f, "MUL"),
            Instruction::Div => write!(f, "DIV"),
            Instruction::IntDiv => write!(f, "INT_DIV"),
            Instruction::Rem => write!(f, "REM"),
            Instruction::Pow => write!(f, "Pow"),
            Instruction::Neg => write!(f, "NEG"),
            Instruction::Eq => write!(f, "EQ"),
            Instruction::Ne => write!(f, "NE"),
            Instruction::Gt => write!(f, "GT"),
            Instruction::Gte => write!(f, "GTE"),
            Instruction::Lt => write!(f, "LT"),
            Instruction::Lte => write!(f, "LTE"),
            Instruction::Not => write!(f, "NOT"),
            Instruction::StringConcat => write!(f, "STRING_CONCAT"),
            Instruction::In => write!(f, "IN"),
            Instruction::ApplyFilter(n) => {
                write!(f, "APPLY_FILTER (name {:?})", n)
            }
            Instruction::PerformTest(n) => {
                write!(f, "PERFORM_TEST (name {:?})", n)
            }
            Instruction::Emit => write!(f, "EMIT"),
            Instruction::PushLoop(flags) => {
                let recursive = flags & LOOP_FLAG_RECURSIVE != 0;
                let loop_var = flags & LOOP_FLAG_WITH_LOOP_VAR != 0;
                write!(
                    f,
                    "PUSH_LOOP (loop var: {:?}, recursive: {:?})",
                    loop_var, recursive
                )
            }
            Instruction::PushWith => write!(f, "PUSH_WITH"),
            Instruction::Iterate(t) => write!(f, "ITERATE (exit to {:>05x})", t),
            Instruction::PopFrame => write!(f, "POP_FRAME"),
            Instruction::Jump(t) => write!(f, "JUMP (to {:>05x})", t),
            Instruction::JumpIfFalse(t) => write!(f, "JUMP_IF_FALSE (to {:>05x})", t),
            Instruction::JumpIfFalseOrPop(t) => write!(f, "JUMP_IF_FALSE_OR_POP (to {:>05x})", t),
            Instruction::JumpIfTrueOrPop(t) => write!(f, "JUMP_IF_TRUE_OR_POP (to {:>05x})", t),
            Instruction::CallBlock(n) => write!(f, "CALL_BLOCK (name {:?})", n),
            Instruction::LoadBlocks => write!(f, "LOAD_BLOCKS"),
            Instruction::Include(b) => write!(f, "INCLUDE (ignore missing {:?})", b),
            Instruction::PushAutoEscape => write!(f, "PUSH_AUTO_ESCAPE"),
            Instruction::PopAutoEscape => write!(f, "POP_AUTO_ESCAPE"),
            Instruction::BeginCapture => write!(f, "BEGIN_CAPTURE"),
            Instruction::EndCapture => write!(f, "END_CAPTURE"),
            Instruction::CallFunction(n) => write!(f, "CALL_FUNCTION (name {:?})", n),
            Instruction::CallMethod(n) => write!(f, "CALL_METHOD (name {:?})", n),
            Instruction::CallObject => write!(f, "CALL_OBJECT"),
            Instruction::DupTop => write!(f, "DUP_TOP"),
            Instruction::DiscardTop => write!(f, "DISCARD_TOP"),
            Instruction::FastSuper => write!(f, "FAST_SUPER"),
            Instruction::FastRecurse => write!(f, "FAST_RECURSE"),
            Instruction::Nop => write!(f, "NOP"),
        }
    }
}

#[derive(Copy, Clone)]
struct Loc {
    first_instruction: u32,
    line: u32,
}

/// Wrapper around instructions to help with location management.
#[derive(Default, Clone)]
pub struct Instructions<'source> {
    pub(crate) instructions: Vec<Instruction<'source>>,
    locations: Vec<Loc>,
    name: &'source str,
    source: &'source str,
}

impl<'source> Instructions<'source> {
    /// Creates a new instructions object.
    pub fn new(name: &'source str, source: &'source str) -> Instructions<'source> {
        Instructions {
            instructions: Vec::new(),
            locations: Vec::new(),
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

    /// Adds a new instruction with location info.
    pub fn add_with_location(&mut self, instr: Instruction<'source>, line: usize) -> usize {
        let rv = self.add(instr);
        let same_loc = self
            .locations
            .last()
            .map_or(false, |last_loc| last_loc.line as usize == line);
        if !same_loc {
            self.locations.push(Loc {
                first_instruction: rv as u32,
                line: line as u32,
            });
        }
        rv
    }

    /// Looks up the line for an instruction
    pub fn get_line(&self, idx: usize) -> Option<usize> {
        let loc = match self
            .locations
            .binary_search_by_key(&idx, |x| x.first_instruction as usize)
        {
            Ok(idx) => &self.locations[idx as usize],
            Err(0) => return None,
            Err(idx) => &self.locations[idx as usize - 1],
        };
        Some(loc.line as usize)
    }

    /// Returns a list of all names referenced in the current block backwards
    /// from the given pc.
    #[cfg(feature = "debug")]
    pub fn get_referenced_names(&self, idx: usize) -> Vec<&'source str> {
        let mut rv = Vec::new();
        for instr in self.instructions[..=idx].iter().rev() {
            let name = match instr {
                Instruction::Lookup(name)
                | Instruction::StoreLocal(name)
                | Instruction::CallFunction(name) => *name,
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
