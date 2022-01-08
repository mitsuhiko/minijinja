use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::instructions::{
    Instruction, Instructions, LOOP_FLAG_RECURSIVE, LOOP_FLAG_WITH_LOOP_VAR,
};
use crate::key::Key;
use crate::utils::matches;
use crate::value::{self, Object, RcType, Value, ValueIterator};
use crate::AutoEscape;

pub struct LoopState {
    len: AtomicUsize,
    idx: AtomicUsize,
    depth: usize,
}

impl fmt::Debug for LoopState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("LoopState");
        for attr in self.attributes() {
            s.field(attr, &self.get_attr(attr).unwrap());
        }
        s.finish()
    }
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
            "depth",
            "depth0",
        ][..]
    }

    fn get_attr(&self, name: &str) -> Option<Value> {
        let idx = self.idx.load(Ordering::Relaxed) as u64;
        let len = self.len.load(Ordering::Relaxed) as u64;
        match name {
            "index0" => Some(Value::from(idx)),
            "index" => Some(Value::from(idx + 1)),
            "length" => Some(Value::from(len)),
            "revindex" => Some(Value::from(len.saturating_sub(idx))),
            "revindex0" => Some(Value::from(len.saturating_sub(idx).saturating_sub(1))),
            "first" => Some(Value::from(idx == 0)),
            "last" => Some(Value::from(len == 0 || idx == len - 1)),
            "depth" => Some(Value::from(self.depth + 1)),
            "depth0" => Some(Value::from(self.depth)),
            _ => None,
        }
    }

    fn call(&self, _state: &State, _args: Vec<Value>) -> Result<Value, Error> {
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "loop cannot be called if reassigned to different variable",
        ))
    }

    fn call_method(&self, _state: &State, name: &str, args: Vec<Value>) -> Result<Value, Error> {
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
        write!(
            f,
            "<loop {}/{}>",
            self.idx.load(Ordering::Relaxed),
            self.len.load(Ordering::Relaxed)
        )
    }
}

#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub struct Loop<'env> {
    locals: BTreeMap<&'env str, Value>,
    with_loop_var: bool,
    recurse_jump_target: Option<usize>,
    current_recursion_jump: Option<usize>,
    iterator: ValueIterator,
    controller: RcType<LoopState>,
}

pub enum Frame<'env, 'vm> {
    // This layer dispatches to another context
    Chained { base: &'vm Context<'env, 'vm> },
    // this layer isolates
    Isolate { value: Value },
    // this layer shadows another one
    Merge { value: Value },
    // this layer is a for loop
    Loop(Loop<'env>),
}

#[cfg(feature = "internal_debug")]
impl<'env, 'vm> fmt::Debug for Frame<'env, 'vm> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chained { base } => fmt::Debug::fmt(base, f),
            Self::Isolate { value } => fmt::Debug::fmt(value, f),
            Self::Merge { value } => fmt::Debug::fmt(value, f),
            Self::Loop(l) => {
                let mut m = f.debug_map();
                m.entries(l.locals.iter());
                m.entry(&"loop", &l.controller);
                m.finish()
            }
        }
    }
}

#[cfg_attr(feature = "internal_debug", derive(Debug))]
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

#[derive(Default)]
#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub struct Context<'env, 'vm> {
    stack: Vec<Frame<'env, 'vm>>,
}

impl<'env, 'vm> Context<'env, 'vm> {
    /// Freezes the context.
    ///
    /// This implementation is not particularly beautiful and highly inefficient.
    /// Since it's only used for the debug support changing this is not too
    /// critical.
    #[cfg(feature = "debug")]
    fn freeze<'a>(&'a self, env: &'a Environment) -> BTreeMap<&'a str, Value> {
        let mut rv = BTreeMap::new();

        for ctx in self.stack.iter() {
            let (lookup_base, cont) = match ctx {
                // if we hit a chain frame we dispatch there and never
                // recurse
                Frame::Chained { base } => return base.freeze(env),
                Frame::Isolate { value } => (value, false),
                Frame::Merge { value } => (value, true),
                Frame::Loop(Loop {
                    locals,
                    controller,
                    with_loop_var,
                    ..
                }) => {
                    rv.extend(locals.iter().map(|(k, v)| (*k, v.clone())));
                    if *with_loop_var {
                        rv.insert("loop", Value::from_rc_object(controller.clone()));
                    }
                    continue;
                }
            };

            if !cont {
                rv.clear();
                rv.extend(env.globals.iter().map(|(k, v)| (*k, v.clone())));
            }

            rv.extend(lookup_base.iter_as_str_map());
        }

        rv
    }

    /// Stores a variable in the context.
    pub fn store(&mut self, key: &'env str, value: Value) {
        self.current_loop()
            .expect("can only assign to loop but not inside a loop")
            .locals
            .insert(key, value);
    }

    /// Looks up a variable in the context.
    pub fn load(&self, env: &Environment, key: &str) -> Option<Value> {
        for ctx in self.stack.iter().rev() {
            let (lookup_base, cont) = match ctx {
                // if we hit a chain frame we dispatch there and never
                // recurse
                Frame::Chained { base } => return base.load(env, key),
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
    pub fn push_frame(&mut self, layer: Frame<'env, 'vm>) {
        self.stack.push(layer);
    }

    /// Pops the topmost layer.
    pub fn pop_frame(&mut self) -> Frame {
        self.stack.pop().expect("pop from empty context stack")
    }

    /// Returns the current innermost loop.
    pub fn current_loop(&mut self) -> Option<&mut Loop<'env>> {
        self.stack
            .iter_mut()
            .rev()
            .filter_map(|x| match *x {
                Frame::Loop(ref mut x) => Some(x),
                _ => None,
            })
            .next()
    }
}

/// Provides access to the current execution state of the engine.
///
/// A read only reference is passed to filter functions and similar objects to
/// allow limited interfacing with the engine.  The state is useful to look up
/// information about the engine in filter, test or global functions.  It not
/// only provides access to the template environment but also the context
/// variables of the engine, the current auto escaping behavior as well as the
/// auto escape flag.
pub struct State<'vm, 'env> {
    pub(crate) env: &'env Environment<'env>,
    pub(crate) ctx: Context<'env, 'vm>,
    pub(crate) name: &'env str,
    pub(crate) current_block: Option<&'env str>,
    pub(crate) auto_escape: AutoEscape,
}

impl<'vm, 'env> fmt::Debug for State<'vm, 'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("State");
        #[cfg(feature = "internal_debug")]
        {
            ds.field("ctx", &self.ctx);
        }
        ds.field("name", &self.name);
        ds.field("current_block", &self.current_block);
        ds.field("auto_escape", &self.auto_escape);
        ds.field("env", &self.env);
        ds.finish()
    }
}

impl<'vm, 'env> State<'vm, 'env> {
    /// Returns a reference to the current environment.
    pub fn env(&self) -> &Environment<'env> {
        self.env
    }

    /// Returns the name of the current template.
    pub fn name(&self) -> &str {
        self.name
    }

    /// Returns the current value of the auto escape flag.
    pub fn auto_escape(&self) -> AutoEscape {
        self.auto_escape
    }

    /// Returns the name of the innermost block.
    pub fn current_block(&self) -> Option<&str> {
        self.current_block
    }

    /// Looks up a variable by name in the context.
    pub fn lookup(&self, name: &str) -> Option<Value> {
        self.ctx.load(self.env(), name)
    }

    pub(crate) fn apply_filter(
        &self,
        name: &str,
        value: Value,
        args: Vec<Value>,
    ) -> Result<Value, Error> {
        if let Some(filter) = self.env().get_filter(name) {
            filter.apply_to(self, value, args)
        } else {
            Err(Error::new(
                ErrorKind::UnknownFilter,
                format!("filter {} is unknown", name),
            ))
        }
    }

    pub(crate) fn perform_test(
        &self,
        name: &str,
        value: Value,
        args: Vec<Value>,
    ) -> Result<bool, Error> {
        if let Some(test) = self.env().get_test(name) {
            test.perform(self, value, args)
        } else {
            Err(Error::new(
                ErrorKind::UnknownTest,
                format!("test {} is unknown", name),
            ))
        }
    }

    #[cfg(feature = "debug")]
    fn make_debug_info(
        &self,
        pc: usize,
        instructions: &Instructions<'_>,
    ) -> crate::error::DebugInfo {
        let referenced_names = instructions.get_referenced_names(pc);
        crate::error::DebugInfo {
            template_source: Some(instructions.source().to_string()),
            context: Some(Value::from(self.ctx.freeze(self.env))),
            referenced_names: Some(referenced_names.iter().map(|x| x.to_string()).collect()),
        }
    }
}

/// Helps to evaluate something.
#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub struct Vm<'env> {
    env: &'env Environment<'env>,
}

impl<'env> Vm<'env> {
    /// Creates a new VM.
    pub fn new(env: &'env Environment<'env>) -> Vm<'env> {
        Vm { env }
    }

    /// Evaluates the given inputs
    pub fn eval(
        &self,
        instructions: &Instructions<'env>,
        root: Value,
        blocks: &BTreeMap<&'env str, Instructions<'env>>,
        initial_auto_escape: AutoEscape,
        output: &mut String,
    ) -> Result<Option<Value>, Error> {
        let mut ctx = Context::default();
        ctx.push_frame(Frame::Isolate { value: root });
        let mut referenced_blocks = BTreeMap::new();
        for (&name, instr) in blocks.iter() {
            referenced_blocks.insert(name, vec![instr]);
        }
        let mut state = State {
            env: self.env,
            ctx,
            auto_escape: initial_auto_escape,
            current_block: None,
            name: instructions.name(),
        };
        value::with_value_optimization(|| {
            self.eval_state(&mut state, instructions, referenced_blocks, output)
        })
    }

    /// This is the actual evaluation loop that works with a specific context.
    fn eval_state(
        &self,
        state: &mut State<'_, 'env>,
        mut instructions: &Instructions<'env>,
        mut blocks: BTreeMap<&'env str, Vec<&'_ Instructions<'env>>>,
        output: &mut String,
    ) -> Result<Option<Value>, Error> {
        let initial_auto_escape = state.auto_escape;
        let mut stack = Stack { values: Vec::new() };
        let mut auto_escape_stack = vec![];
        let mut capture_stack = vec![];
        let mut block_stack = vec![];
        let mut next_loop_recursion_jump = None;
        let mut pc = 0;

        macro_rules! bail {
            ($err:expr) => {{
                let mut err = $err;
                if let Some(lineno) = instructions.get_line(pc) {
                    err.set_location(instructions.name(), lineno);
                }
                #[cfg(feature = "debug")]
                {
                    if self.env.debug() && err.debug_info.is_none() {
                        err.debug_info = Some(state.make_debug_info(pc, &instructions));
                    }
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
                stack.push(if !matches!(state.auto_escape, AutoEscape::None) {
                    Value::from_safe_string(captured)
                } else {
                    Value::from(captured)
                });
            }};
        }

        macro_rules! sub_eval {
            ($instructions:expr) => {{
                sub_eval!(
                    $instructions,
                    blocks.clone(),
                    state.current_block,
                    state.auto_escape
                );
            }};
            ($instructions:expr, $blocks:expr, $current_block:expr, $auto_escape:expr) => {{
                let mut sub_context = Context::default();
                sub_context.push_frame(Frame::Chained { base: &state.ctx });
                let mut sub_state = State {
                    env: self.env,
                    ctx: sub_context,
                    auto_escape: $auto_escape,
                    current_block: $current_block,
                    name: $instructions.name(),
                };
                self.eval_state(&mut sub_state, $instructions, $blocks, out!())?;
            }};
        }

        while let Some(instr) = instructions.get(pc) {
            match instr {
                Instruction::EmitRaw(val) => {
                    write!(out!(), "{}", val).unwrap();
                }
                Instruction::Emit => {
                    try_ctx!(self.env.finalize(&stack.pop(), state.auto_escape, out!()));
                }
                Instruction::StoreLocal(name) => {
                    state.ctx.store(name, stack.pop());
                }
                Instruction::Lookup(name) => {
                    stack.push(state.ctx.load(self.env, name).unwrap_or(Value::UNDEFINED));
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
                        let key: Key = try_ctx!(stack.pop().try_into_key());
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
                    let mut v = try_ctx!(stack.pop().try_into_vec().map_err(|e| {
                        Error::new(
                            ErrorKind::ImpossibleOperation,
                            "cannot unpack: not a sequence",
                        )
                        .with_source(e)
                    }));
                    if v.len() != *count {
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
                            format!(
                                "cannot unpack: sequence of wrong length (expected {}, got {})",
                                *count,
                                v.len()
                            )
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
                Instruction::IntDiv => func_binop!(int_div),
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
                    state.ctx.push_frame(Frame::Merge { value });
                }
                Instruction::PopFrame => {
                    if let Frame::Loop(mut loop_ctx) = state.ctx.pop_frame() {
                        if let Some(target) = loop_ctx.current_recursion_jump.take() {
                            pc = target;
                            end_capture!();
                            continue;
                        }
                    }
                }
                Instruction::PushLoop(flags) => {
                    let iterable = stack.pop();
                    let iterator = iterable.iter();
                    let len = iterator.len();
                    let depth = state
                        .ctx
                        .current_loop()
                        .filter(|x| x.recurse_jump_target.is_some())
                        .map_or(0, |x| x.controller.depth + 1);
                    let recursive = *flags & LOOP_FLAG_RECURSIVE != 0;
                    state.ctx.push_frame(Frame::Loop(Loop {
                        locals: BTreeMap::new(),
                        iterator,
                        with_loop_var: *flags & LOOP_FLAG_WITH_LOOP_VAR != 0,
                        recurse_jump_target: if recursive { Some(pc) } else { None },
                        current_recursion_jump: next_loop_recursion_jump.take(),
                        controller: RcType::new(LoopState {
                            idx: AtomicUsize::new(!0usize),
                            len: AtomicUsize::new(len),
                            depth,
                        }),
                    }));
                }
                Instruction::Iterate(jump_target) => {
                    let l = state.ctx.current_loop().expect("not inside a loop");
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
                    block_stack.push(state.current_block);
                    state.current_block = Some(name);
                    if let Some(layers) = blocks.get(name) {
                        let instructions = layers.first().unwrap();
                        sub_eval!(instructions);
                    } else {
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
                            "tried to invoke unknown block"
                        ));
                    }
                    state.current_block = block_stack.pop().unwrap();
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
                    state.name = instructions.name();
                    pc = 0;
                    continue;
                }
                Instruction::Include => {
                    let name = stack.pop();
                    let name = try_ctx!(name.as_str().ok_or_else(|| {
                        Error::new(
                            ErrorKind::ImpossibleOperation,
                            "template name was not a string",
                        )
                    }));
                    let tmpl = try_ctx!(self.env.get_template(name));
                    let instructions = tmpl.instructions();
                    let mut referenced_blocks = BTreeMap::new();
                    for (&name, instr) in tmpl.blocks().iter() {
                        referenced_blocks.insert(name, vec![instr]);
                    }
                    sub_eval!(
                        instructions,
                        referenced_blocks,
                        None,
                        tmpl.initial_auto_escape()
                    );
                }
                Instruction::PushAutoEscape => {
                    let value = stack.pop();
                    auto_escape_stack.push(state.auto_escape);
                    state.auto_escape = match (value.as_str(), value == Value::from(true)) {
                        (Some("html"), _) => AutoEscape::Html,
                        (Some("none"), _) | (None, false) => AutoEscape::None,
                        (None, true) => {
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
                    state.auto_escape = auto_escape_stack.pop().unwrap();
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
                    stack.push(try_ctx!(state.apply_filter(name, value, args)));
                }
                Instruction::PerformTest(name) => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let value = stack.pop();
                    stack.push(Value::from(try_ctx!(state.perform_test(name, value, args))));
                }
                Instruction::CallFunction(function_name) => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    // super is a special function reserved for super-ing into blocks.
                    if *function_name == "super" {
                        if !args.is_empty() {
                            bail!(Error::new(
                                ErrorKind::ImpossibleOperation,
                                "super() takes no arguments",
                            ));
                        }
                        let mut inner_blocks = blocks.clone();
                        let name = match state.current_block {
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
                    // loop is a special name which when called recurses the current loop.
                    } else if *function_name == "loop" {
                        if let Some(loop_ctx) = state.ctx.current_loop() {
                            if args.len() != 1 {
                                bail!(Error::new(
                                    ErrorKind::ImpossibleOperation,
                                    format!("loop() takes one argument, got {}", args.len())
                                ));
                            }
                            if let Some(recurse_jump_target) = loop_ctx.recurse_jump_target {
                                // the way this works is that we remember the next instruction
                                // as loop exit jump target.  Whenever a loop is pushed, it
                                // memorizes the value in `next_loop_iteration_jump` to jump
                                // to and also end the current capture.
                                next_loop_recursion_jump = Some(pc + 1);
                                stack.push(args.into_iter().next().unwrap());
                                pc = recurse_jump_target;
                                begin_capture!();
                                continue;
                            } else {
                                bail!(Error::new(
                                    ErrorKind::ImpossibleOperation,
                                    "cannot recurse outside of recursive loop"
                                ));
                            }
                        } else {
                            bail!(Error::new(
                                ErrorKind::ImpossibleOperation,
                                "tried to recurse outside of loop"
                            ));
                        }
                    } else if let Some(func) = state.ctx.load(self.env, function_name) {
                        stack.push(try_ctx!(func.call(state, args)));
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
                    stack.push(try_ctx!(obj.call_method(state, name, args)));
                }
                Instruction::CallObject => {
                    let args = try_ctx!(stack.pop().try_into_vec());
                    let obj = stack.pop();
                    stack.push(try_ctx!(obj.call(state, args)));
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
pub fn simple_eval<S: serde::Serialize>(
    instructions: &Instructions<'_>,
    ctx: S,
    output: &mut String,
) -> Result<Option<Value>, Error> {
    let env = Environment::new();
    let empty_blocks = BTreeMap::new();
    let vm = Vm::new(&env);
    let root = Value::from_serializable(&ctx);
    vm.eval(instructions, root, &empty_blocks, AutoEscape::None, output)
}
