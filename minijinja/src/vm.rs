use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::instructions::{Instruction, Instructions};
use crate::key::Key;
use crate::utils::matches;
use crate::value::{self, Object, Primitive, RcType, Value, ValueIterator};
use crate::AutoEscape;

#[derive(Debug)]
pub struct LoopState {
    len: AtomicUsize,
    idx: AtomicUsize,
}

impl Object for LoopState {
    fn attributes(&self) -> &[&str] {
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

    fn call_method(
        &self,
        _env: &Environment,
        name: &str,
        args: Vec<Value>,
    ) -> Result<Value, Error> {
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
pub struct Loop<'env> {
    locals: BTreeMap<&'env str, Value>,
    with_loop_var: bool,
    iterator: ValueIterator,
    controller: RcType<LoopState>,
}

#[derive(Debug)]
pub enum Frame<'env, 'context> {
    // This layer dispatches to another context
    Chained {
        base: &'context Context<'env, 'context>,
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
    Loop(Loop<'env>),
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
pub struct Context<'env, 'context> {
    stack: Vec<Frame<'env, 'context>>,
}

impl<'env, 'context> Context<'env, 'context> {
    /// Stores a variable in the context.
    pub fn store(&mut self, key: &'env str, value: Value) {
        self.current_loop().locals.insert(key, value);
    }

    /// Looks up a variable in the context.
    pub fn lookup(&self, env: &Environment, key: &str) -> Option<Value> {
        for ctx in self.stack.iter().rev() {
            let (lookup_base, cont) = match ctx {
                // if we hit a chain frame we dispatch there and never
                // recurse
                Frame::Chained { base } => return base.lookup(env, key),
                Frame::Isolate { value } => (value, false),
                Frame::Merge { value } => (value, true),
                Frame::Loop(Loop {
                    locals,
                    controller,
                    with_loop_var,
                    ..
                }) => {
                    if *with_loop_var && key == "loop" {
                        return Some(Value::from_rc_object(controller.clone()));
                    } else if let Some(value) = locals.get(key) {
                        return Some(value.clone());
                    }
                    continue;
                }
            };

            let rv = lookup_base.get_attr(key);
            if let Ok(rv) = rv {
                if !rv.is_undefined() {
                    return Some(rv);
                }
            }
            if !cont {
                if let Some(value) = env.get_global(key) {
                    return Some(value);
                }
                break;
            }
        }
        None
    }

    /// Pushes a new layer.
    pub fn push_frame(&mut self, layer: Frame<'env, 'context>) {
        self.stack.push(layer);
    }

    /// Pops the topmost layer.
    pub fn pop_frame(&mut self) -> Frame {
        self.stack.pop().expect("pop from empty context stack")
    }

    /// Returns the current innermost loop.
    pub fn current_loop(&mut self) -> &mut Loop<'env> {
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
pub struct Vm<'env> {
    env: &'env Environment<'env>,
}

impl<'env> Vm<'env> {
    /// Creates a new VM.
    pub fn new(env: &'env Environment<'env>) -> Vm<'env> {
        Vm { env }
    }

    /// Evaluates the given inputs
    pub fn eval<S: Serialize>(
        &self,
        instructions: &Instructions<'env>,
        root: S,
        blocks: &BTreeMap<&'env str, Instructions<'env>>,
        initial_auto_escape: AutoEscape,
        output: &mut String,
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
    fn eval_context(
        &self,
        mut instructions: &'env Instructions<'env>,
        context: &mut Context<'env, '_>,
        blocks: &BTreeMap<&'env str, Vec<&'env Instructions<'env>>>,
        block_stack: &mut Vec<&'env str>,
        initial_auto_escape: AutoEscape,
        output: &mut String,
    ) -> Result<Option<Value>, Error> {
        let mut pc = 0;
        let mut stack = Stack::default();
        let mut blocks = blocks.clone();
        let mut auto_escape = initial_auto_escape;
        let mut auto_escape_stack = vec![];
        let mut capture_stack = vec![];

        macro_rules! bail {
            ($err:expr) => {{
                let mut err = $err;
                if let Some((filename, lineno)) = instructions.get_location(pc) {
                    err.set_location(filename, lineno);
                }
                return Err(err);
            }};
        }

        macro_rules! try_ctx {
            ($expr:expr) => {
                match $expr {
                    Ok(rv) => rv,
                    Err(err) => bail!(err),
                }
            };
        }

        macro_rules! func_binop {
            ($method:ident) => {{
                let b = stack.pop();
                let a = stack.pop();
                stack.push(try_ctx!(value::$method(&a, &b)));
            }};
        }

        macro_rules! op_binop {
            ($op:tt) => {{
                let b = stack.pop();
                let a = stack.pop();
                stack.push(Value::from(a $op b));
            }};
        }

        macro_rules! out {
            () => {
                capture_stack.last_mut().unwrap_or(output)
            };
        }

        macro_rules! begin_capture {
            () => {
                capture_stack.push(String::new());
            };
        }

        macro_rules! end_capture {
            () => {{
                let captured = capture_stack.pop().unwrap();
                // TODO: this should take the right auto escapine flag into account
                stack.push(if !matches!(auto_escape, AutoEscape::None) {
                    Value::from_safe_string(captured)
                } else {
                    Value::from(captured)
                });
            }};
        }

        macro_rules! sub_eval {
            ($instructions:expr) => {{
                sub_eval!($instructions, &blocks, block_stack, auto_escape);
            }};
            ($instructions:expr, $blocks:expr, $block_stack:expr, $auto_escape:expr) => {{
                let mut sub_context = Context::default();
                sub_context.push_frame(Frame::Chained { base: context });
                let sub_vm = Vm::new(self.env);
                sub_vm.eval_context(
                    $instructions,
                    &mut sub_context,
                    $blocks,
                    $block_stack,
                    $auto_escape,
                    out!(),
                )?;
            }};
        }

        while let Some(instr) = instructions.get(pc) {
            match instr {
                Instruction::EmitRaw(val) => {
                    write!(out!(), "{}", val).unwrap();
                }
                Instruction::Emit => {
                    try_ctx!(self.env.finalize(&stack.pop(), auto_escape, out!()));
                }
                Instruction::StoreLocal(name) => {
                    context.store(name, stack.pop());
                }
                Instruction::Lookup(name) => {
                    stack.push(context.lookup(self.env, name).unwrap_or(Value::UNDEFINED));
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
                Instruction::UnpackList(count) => {
                    let mut v = try_ctx!(stack.pop().try_into_vec());
                    if v.len() != *count {
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
                            "sequence of wrong length"
                        ));
                    }
                    for _ in 0..*count {
                        stack.push(v.pop().unwrap());
                    }
                }
                Instruction::ListAppend => {
                    let item = stack.pop();
                    let mut list = try_ctx!(stack.pop().try_into_vec());
                    list.push(item);
                    stack.push(Value::from(list));
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
                Instruction::In => {
                    let container = stack.pop();
                    let value = stack.pop();
                    stack.push(try_ctx!(value::contains(&container, &value)));
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
                Instruction::PushLoop(with_loop_var) => {
                    let iterable = stack.pop();
                    let iterator = iterable.iter();
                    let len = iterator.len();
                    context.push_frame(Frame::Loop(Loop {
                        locals: BTreeMap::new(),
                        iterator,
                        with_loop_var: *with_loop_var,
                        controller: RcType::new(LoopState {
                            idx: AtomicUsize::new(!0usize),
                            len: AtomicUsize::new(len),
                        }),
                    }));
                }
                Instruction::Iterate(jump_target) => {
                    let l = context.current_loop();
                    l.controller.idx.fetch_add(1, Ordering::Relaxed);
                    match l.iterator.next() {
                        Some(item) => {
                            stack.push(item);
                        }
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
                        continue;
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
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
                            "tried to invoke unknown block"
                        ));
                    }
                    block_stack.pop();
                }
                Instruction::LoadBlocks => {
                    let name = stack.pop();
                    let tmpl = try_ctx!(name
                        .as_str()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::ImpossibleOperation,
                                "template name was not a string",
                            )
                        })
                        .and_then(|name| self.env.get_template(name)));

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
                Instruction::Include => {
                    let name = stack.pop();
                    let tmpl = try_ctx!(name
                        .as_str()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::ImpossibleOperation,
                                "template name was not a string",
                            )
                        })
                        .and_then(|name| self.env.get_template(name)));
                    let instructions = tmpl.instructions();
                    let mut referenced_blocks = BTreeMap::new();
                    for (&name, instr) in tmpl.blocks().iter() {
                        referenced_blocks.insert(name, vec![instr]);
                    }
                    let mut block_stack = Vec::new();
                    sub_eval!(
                        instructions,
                        &referenced_blocks,
                        &mut block_stack,
                        tmpl.initial_auto_escape()
                    );
                }
                Instruction::PushAutoEscape => {
                    let value = stack.pop();
                    auto_escape_stack.push(auto_escape);
                    auto_escape = match value.as_primitive() {
                        Some(Primitive::Str("html")) => AutoEscape::Html,
                        Some(Primitive::Str("none")) | Some(Primitive::Bool(false)) => {
                            AutoEscape::None
                        }
                        Some(Primitive::Bool(true)) => {
                            if matches!(initial_auto_escape, AutoEscape::None) {
                                AutoEscape::Html
                            } else {
                                initial_auto_escape
                            }
                        }
                        _ => {
                            bail!(Error::new(
                                ErrorKind::ImpossibleOperation,
                                "invalid value to autoescape tag",
                            ));
                        }
                    };
                }
                Instruction::PopAutoEscape => {
                    auto_escape = auto_escape_stack.pop().unwrap();
                }
                Instruction::BeginCapture => {
                    begin_capture!();
                }
                Instruction::EndCapture => {
                    end_capture!();
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
                    let args = try_ctx!(stack.pop().try_into_vec());
                    // super is a special function reserved for super-ing into blocks.
                    if *function_name == "super" {
                        let mut inner_blocks = blocks.clone();
                        let name = match block_stack.last() {
                            Some(name) => name,
                            None => {
                                bail!(Error::new(
                                    ErrorKind::ImpossibleOperation,
                                    "cannot super outside of block",
                                ));
                            }
                        };
                        if let Some(layers) = inner_blocks.get_mut(name) {
                            layers.remove(0);
                            let instructions = layers.first().unwrap();
                            begin_capture!();
                            sub_eval!(instructions);
                            end_capture!();
                        } else {
                            panic!("attempted to super unreferenced block");
                        }
                    } else if let Some(func) = context.lookup(self.env, function_name) {
                        stack.push(try_ctx!(func.call(self.env, args)));
                    } else {
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
                            format!("unknown function {}", function_name),
                        ));
                    }
                }
                Instruction::CallMethod(name) => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let obj = stack.pop();
                    stack.push(try_ctx!(obj.call_method(self.env, name, args)));
                }
                Instruction::CallObject => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let obj = stack.pop();
                    stack.push(try_ctx!(obj.call(self.env, args)));
                }
                Instruction::DupTop => {
                    stack.push(stack.peek().clone());
                }
                Instruction::DiscardTop => {
                    stack.pop();
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
pub fn simple_eval<S: Serialize>(
    instructions: &Instructions<'_>,
    root: S,
    output: &mut String,
) -> Result<Option<Value>, Error> {
    let env = Environment::new();
    let empty_blocks = BTreeMap::new();
    let vm = Vm::new(&env);
    vm.eval(instructions, root, &empty_blocks, AutoEscape::None, output)
}
