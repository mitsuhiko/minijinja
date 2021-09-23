use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::instructions::{Instruction, Instructions};
use crate::key::Key;
use crate::value::{self, DynamicObject, Primitive, RcType, Value, ValueIterator};
use crate::AutoEscape;

#[derive(Debug)]
pub struct LoopState {
    len: AtomicUsize,
    idx: AtomicUsize,
}

impl DynamicObject for LoopState {
    fn fields(&self) -> &'static [&'static str] {
        &[
            "index0",
            "index",
            "length",
            "revindex",
            "revindex0",
            "first",
            "last",
        ][..]
    }

    fn get_attr(&self, name: &str) -> Option<Value> {
        let idx = self.idx.load(Ordering::Relaxed) as u64;
        let len = self.len.load(Ordering::Relaxed) as u64;
        match name {
            "index0" => Some(Value::from(idx)),
            "index" => Some(Value::from(idx + 1)),
            "length" => Some(Value::from(len)),
            "revindex" => Some(Value::from(len - idx)),
            "revindex0" => Some(Value::from(len - idx - 1)),
            "first" => Some(Value::from(idx == 0)),
            "last" => Some(Value::from(idx == len - 1)),
            _ => None,
        }
    }

    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, Error> {
        if name == "cycle" {
            let idx = self.idx.load(Ordering::Relaxed);
            match args.get(idx % args.len()) {
                Some(arg) => Ok(arg.clone()),
                None => Ok(Value::UNDEFINED),
            }
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!("loop object has no method named {}", name),
            ))
        }
    }
}

impl fmt::Display for LoopState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "loop")
    }
}

#[derive(Debug)]
pub struct Loop<'source> {
    target_name: &'source str,
    current_value: Value,
    iterator: ValueIterator,
    controller: RcType<LoopState>,
}

#[derive(Debug)]
pub enum Frame<'source, 'context> {
    // This layer dispatches to another context
    Chained {
        base: &'context Context<'source, 'context>,
    },
    // this layer isolates
    Isolate {
        value: Value,
    },
    // this layer shadows another one
    Merge {
        value: Value,
    },
    // this layer is a for loop
    Loop(Loop<'source>),
}

#[derive(Debug, Default)]
pub struct Stack {
    values: Vec<Value>,
}

impl Stack {
    pub fn push(&mut self, arg: Value) {
        self.values.push(arg);
    }

    pub fn pop(&mut self) -> Value {
        self.values.pop().expect("stack was empty")
    }

    pub fn try_pop(&mut self) -> Option<Value> {
        self.values.pop()
    }

    pub fn peek(&self) -> &Value {
        self.values.last().expect("stack was empty")
    }
}

#[derive(Default, Debug)]
pub struct Context<'source, 'context> {
    stack: Vec<Frame<'source, 'context>>,
}

impl<'source, 'context> Context<'source, 'context> {
    /// Looks up a variable in the context.
    pub fn lookup(&self, key: &str) -> Option<Value> {
        for ctx in self.stack.iter().rev() {
            let (lookup_base, cont) = match ctx {
                // if we hit a chain frame we dispatch there and never
                // recurse
                Frame::Chained { base } => return base.lookup(key),
                Frame::Isolate { value } => (value, false),
                Frame::Merge { value } => (value, true),
                Frame::Loop(Loop {
                    target_name,
                    current_value,
                    controller,
                    ..
                }) => {
                    if key == *target_name {
                        return Some(current_value.clone());
                    } else if key == "loop" {
                        return Some(Value::from_dynamic(controller.clone()));
                    }
                    continue;
                }
            };

            let rv = lookup_base.get_attr(key);
            if let Ok(rv) = rv {
                return Some(rv);
            } else if !cont {
                break;
            }
        }
        None
    }

    /// Pushes a new layer.
    pub fn push_frame(&mut self, layer: Frame<'source, 'context>) {
        self.stack.push(layer);
    }

    /// Pops the topmost layer.
    pub fn pop_frame(&mut self) -> Frame {
        self.stack.pop().expect("pop from empty context stack")
    }

    /// Returns the current innermost loop.
    pub fn current_loop(&mut self) -> &mut Loop<'source> {
        self.stack
            .iter_mut()
            .rev()
            .filter_map(|x| match *x {
                Frame::Loop(ref mut x) => Some(x),
                _ => None,
            })
            .next()
            .expect("not inside a loop")
    }
}

/// Helps to evaluate something.
#[derive(Debug)]
pub struct Vm<'env, 'source> {
    env: &'env Environment<'source>,
}

impl<'env, 'source> Vm<'env, 'source> {
    /// Creates a new VM.
    pub fn new(env: &'env Environment<'source>) -> Vm<'env, 'source> {
        Vm { env }
    }

    /// Evaluates the given inputs
    pub fn eval<W: Write, S: Serialize>(
        &self,
        instructions: &Instructions<'source>,
        root: S,
        blocks: &BTreeMap<&'source str, Instructions<'source>>,
        initial_auto_escape: AutoEscape,
        output: &mut W,
    ) -> Result<Option<Value>, Error> {
        let mut context = Context::default();
        let root = Value::from_serializable(&root);
        context.push_frame(Frame::Isolate { value: root });
        let mut referenced_blocks = BTreeMap::new();
        for (&name, instr) in blocks.iter() {
            referenced_blocks.insert(name, vec![instr]);
        }
        let mut block_stack = vec![];
        self.eval_context(
            instructions,
            &mut context,
            &referenced_blocks,
            &mut block_stack,
            initial_auto_escape,
            output,
        )
    }

    /// This is the actual evaluation loop that works with a specific context.
    fn eval_context<'context, W: Write>(
        &self,
        mut instructions: &'env Instructions<'source>,
        context: &'context mut Context<'source, 'context>,
        blocks: &BTreeMap<&'source str, Vec<&'env Instructions<'source>>>,
        block_stack: &mut Vec<&'source str>,
        initial_auto_escape: AutoEscape,
        output: &mut W,
    ) -> Result<Option<Value>, Error>
    where
        'source: 'context,
        'env: 'context,
    {
        let mut pc = 0;
        let mut stack = Stack::default();
        let mut blocks = blocks.clone();
        let mut auto_escape = initial_auto_escape;
        let mut auto_escape_stack = vec![];

        macro_rules! try_ctx {
            ($expr:expr) => {
                match $expr {
                    Ok(rv) => rv,
                    Err(mut err) => {
                        if let Some((filename, lineno)) = instructions.get_location(pc) {
                            err.set_location(filename, lineno);
                        }
                        return Err(err);
                    }
                }
            };
        }

        macro_rules! func_binop {
            ($method:ident) => {{
                let a = stack.pop();
                let b = stack.pop();
                stack.push(try_ctx!(value::$method(&b, &a)));
            }};
        }

        macro_rules! op_binop {
            ($op:tt) => {{
                let a = stack.pop();
                let b = stack.pop();
                stack.push(Value::from(b $op a));
            }};
        }

        macro_rules! sub_eval {
            ($instructions:expr) => {{
                let mut sub_context = Context::default();
                sub_context.push_frame(Frame::Chained { base: context });
                let sub_vm = Vm::new(self.env);
                sub_vm.eval_context(
                    $instructions,
                    &mut sub_context,
                    &blocks,
                    block_stack,
                    auto_escape,
                    output,
                )?;
            }};
        }

        while let Some(instr) = instructions.get(pc) {
            match instr {
                Instruction::EmitRaw(val) => {
                    write!(output, "{}", val).unwrap();
                }
                Instruction::Emit => {
                    try_ctx!(self.env.finalize(&stack.pop(), auto_escape, output));
                }
                Instruction::Lookup(name) => {
                    stack.push(context.lookup(name).unwrap_or(Value::UNDEFINED));
                }
                Instruction::GetAttr(name) => {
                    let value = stack.pop();
                    stack.push(try_ctx!(value.get_attr(name)));
                }
                Instruction::GetItem => {
                    let attr = stack.pop();
                    let value = stack.pop();
                    stack.push(try_ctx!(value.get_item(&attr)));
                }
                Instruction::LoadConst(value) => {
                    stack.push(value.clone());
                }
                Instruction::BuildMap(pair_count) => {
                    let mut map = BTreeMap::new();
                    for _ in 0..*pair_count {
                        let value = stack.pop();
                        let key: Key = try_ctx!(TryFrom::try_from(stack.pop()));
                        map.insert(key, value);
                    }
                    stack.push(Value::from(map));
                }
                Instruction::BuildList(count) => {
                    let mut v = Vec::new();
                    for _ in 0..*count {
                        v.push(stack.pop());
                    }
                    v.reverse();
                    stack.push(v.into());
                }
                Instruction::Add => func_binop!(add),
                Instruction::Sub => func_binop!(sub),
                Instruction::Mul => func_binop!(mul),
                Instruction::Div => func_binop!(div),
                Instruction::Rem => func_binop!(rem),
                Instruction::Pow => func_binop!(pow),
                Instruction::Eq => op_binop!(==),
                Instruction::Ne => op_binop!(!=),
                Instruction::Gt => op_binop!(>),
                Instruction::Gte => op_binop!(>=),
                Instruction::Lt => op_binop!(<),
                Instruction::Lte => op_binop!(<=),
                Instruction::Not => {
                    let a = stack.pop();
                    stack.push(Value::from(!a.is_true()));
                }
                Instruction::StringConcat => {
                    let a = stack.pop();
                    let b = stack.pop();
                    stack.push(value::string_concat(b, &a));
                }
                Instruction::Neg => {
                    let a = stack.pop();
                    stack.push(try_ctx!(value::neg(&a)));
                }
                Instruction::PushContext => {
                    let value = stack.pop();
                    context.push_frame(Frame::Merge { value });
                }
                Instruction::PopFrame => {
                    context.pop_frame();
                }
                Instruction::PushLoop(target_name) => {
                    let iterable = stack.pop();
                    let iterator = iterable.iter();
                    let len = iterator.len();
                    context.push_frame(Frame::Loop(Loop {
                        target_name,
                        current_value: Value::UNDEFINED,
                        iterator,
                        controller: RcType::new(LoopState {
                            idx: AtomicUsize::new(!0usize),
                            len: AtomicUsize::new(len),
                        }),
                    }));
                }
                Instruction::Iterate(jump_target) => {
                    let l = context.current_loop();
                    l.controller.idx.fetch_add(1, Ordering::Relaxed);
                    l.current_value = match l.iterator.next() {
                        Some(item) => item,
                        None => {
                            pc = *jump_target;
                            continue;
                        }
                    };
                }
                Instruction::Jump(jump_target) => {
                    pc = *jump_target;
                    continue;
                }
                Instruction::JumpIfFalse(jump_target) => {
                    let value = stack.pop();
                    if !value.is_true() {
                        pc = *jump_target;
                        continue;
                    }
                }
                Instruction::JumpIfFalseOrPop(jump_target) => {
                    if !stack.peek().is_true() {
                        pc = *jump_target;
                    } else {
                        stack.pop();
                    }
                }
                Instruction::JumpIfTrueOrPop(jump_target) => {
                    if stack.peek().is_true() {
                        pc = *jump_target;
                    } else {
                        stack.pop();
                    }
                }
                Instruction::CallBlock(name) => {
                    block_stack.push(name);
                    if let Some(layers) = blocks.get(name) {
                        let instructions = layers.first().unwrap();
                        sub_eval!(instructions);
                    } else {
                        panic!("attempted to evaluate unreferenced block");
                    }
                    block_stack.pop();
                }
                Instruction::LoadBlocks => {
                    let name = stack.pop();
                    let tmpl = try_ctx!(name
                        .as_str()
                        .and_then(|name| self.env.get_template(name))
                        .ok_or_else(|| {
                            Error::new(ErrorKind::TemplateNotFound, "could not find template")
                        }));

                    // first load the blocks
                    for (name, instr) in tmpl.blocks().iter() {
                        blocks.entry(name).or_insert_with(Vec::new).push(instr);
                    }

                    // then replace the instructions and set the pc to 0 again.
                    // this effectively means that the template engine will now
                    // execute the extended template's code instead.  From this
                    // there is no way back.
                    instructions = tmpl.instructions();
                    pc = 0;
                    continue;
                }
                Instruction::PushAutoEscape => {
                    let value = stack.pop();
                    auto_escape_stack.push(auto_escape);
                    auto_escape = match value.as_primitive() {
                        Some(Primitive::Str("html")) => AutoEscape::Html,
                        Some(Primitive::Str("none") | Primitive::Bool(false)) => AutoEscape::None,
                        Some(Primitive::Bool(true)) => {
                            if matches!(initial_auto_escape, AutoEscape::None) {
                                AutoEscape::Html
                            } else {
                                initial_auto_escape
                            }
                        }
                        _ => {
                            return Err(Error::new(
                                ErrorKind::ImpossibleOperation,
                                "invalid value to autoescape tag",
                            ));
                        }
                    };
                }
                Instruction::PopAutoEscape => {
                    auto_escape = auto_escape_stack.pop().unwrap();
                }
                Instruction::ApplyFilter(name) => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let value = stack.pop();
                    stack.push(try_ctx!(self.env.apply_filter(name, value, args)));
                }
                Instruction::PerformTest(name) => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let value = stack.pop();
                    stack.push(Value::from(try_ctx!(self
                        .env
                        .perform_test(name, value, args))));
                }
                Instruction::CallFunction(function_name) => {
                    // this is the only function we recognize today and it's
                    // very special.  In fact it is interpreted very similar to how
                    // the block syntax works.
                    if *function_name == "super" {
                        let mut inner_blocks = blocks.clone();
                        let name = block_stack.last().expect("empty block stack");
                        if let Some(layers) = inner_blocks.get_mut(name) {
                            layers.remove(0);
                            let instructions = layers.first().unwrap();
                            sub_eval!(instructions);
                        } else {
                            panic!("attempted to super unreferenced block");
                        }
                    } else {
                        return Err(Error::new(
                            ErrorKind::ImpossibleOperation,
                            format!("unknown function {}", function_name),
                        ));
                    }
                }
                Instruction::CallMethod(name) => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let obj = stack.pop();
                    stack.push(try_ctx!(obj.call_method(name, args)));
                }
                Instruction::CallObject => {
                    let _args = try_ctx!(stack.pop().try_into_vec());
                    let _obj = stack.pop();
                    // TODO: this is something that doesn't make too much sense in the
                    // engine today.
                    return Err(Error::new(
                        ErrorKind::ImpossibleOperation,
                        "objects cannot be called directly",
                    ));
                }
                Instruction::Nop => {}
            }
            pc += 1;
        }

        Ok(stack.try_pop())
    }
}

/// Simple version of eval without environment or vm.
#[cfg(feature = "unstable_machinery")]
pub fn simple_eval<W: Write, S: Serialize>(
    instructions: &Instructions<'_>,
    root: S,
    output: &mut W,
) -> Result<Option<Value>, Error> {
    let env = Environment::new();
    let empty_blocks = BTreeMap::new();
    let vm = Vm::new(&env);
    vm.eval(instructions, root, &empty_blocks, AutoEscape::None, output)
}
