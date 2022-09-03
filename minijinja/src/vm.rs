use std::collections::{BTreeMap, HashSet};
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "sync")]
use std::sync::Mutex;

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::instructions::{
    Instruction, Instructions, LOOP_FLAG_RECURSIVE, LOOP_FLAG_WITH_LOOP_VAR,
};
use crate::key::Key;
use crate::utils::matches;
use crate::value::{self, Object, RcType, Value, ValueIterator, ValueRepr};
use crate::AutoEscape;

pub struct LoopState {
    len: usize,
    idx: AtomicUsize,
    depth: usize,
    #[cfg(feature = "sync")]
    last_changed_value: Mutex<Option<Vec<Value>>>,
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
        let len = self.len as u64;
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

    fn call(&self, _state: &State, _args: &[Value]) -> Result<Value, Error> {
        Err(Error::new(
            ErrorKind::ImpossibleOperation,
            "loop cannot be called if reassigned to different variable",
        ))
    }

    fn call_method(&self, _state: &State, name: &str, args: &[Value]) -> Result<Value, Error> {
        #[cfg(feature = "sync")]
        {
            if name == "changed" {
                let mut last_changed_value = self.last_changed_value.lock().unwrap();
                let value = args.to_owned();
                let changed = last_changed_value.as_ref() != Some(&value);
                if changed {
                    *last_changed_value = Some(value);
                    return Ok(Value::from(true));
                }
                return Ok(Value::from(false));
            }
        }

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
            self.len
        )
    }
}

type Locals<'env> = BTreeMap<&'env str, Value>;

#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub struct Loop {
    with_loop_var: bool,
    recurse_jump_target: Option<usize>,
    // if we're popping the frame, do we want to jump somewhere?  The
    // first item is the target jump instruction, the second argument
    // tells us if we need to end capturing.
    current_recursion_jump: Option<(usize, bool)>,
    iterator: ValueIterator,
    controller: RcType<LoopState>,
}

#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub enum FrameBase<'env, 'vm> {
    None,
    Context(&'vm Context<'env, 'vm>),
    Value(Value),
}

pub struct Frame<'env, 'vm> {
    locals: Locals<'env>,
    base: FrameBase<'env, 'vm>,
    current_loop: Option<Loop>,
}

impl<'env, 'vm> Default for Frame<'env, 'vm> {
    fn default() -> Frame<'env, 'vm> {
        Frame::new(FrameBase::None)
    }
}

impl<'env, 'vm> Frame<'env, 'vm> {
    pub fn new(base: FrameBase<'env, 'vm>) -> Frame<'env, 'vm> {
        Frame {
            locals: Locals::new(),
            base,
            current_loop: None,
        }
    }
}

#[cfg(feature = "internal_debug")]
impl<'env, 'vm> fmt::Debug for Frame<'env, 'vm> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        m.entries(self.locals.iter());
        if let Some(Loop { ref controller, .. }) = self.current_loop {
            m.entry(&"loop", controller);
        }
        if let FrameBase::Value(ref value) = self.base {
            m.entries(value.iter_as_str_map());
        }
        m.finish()
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
pub struct Context<'env, 'vm> {
    stack: Vec<Frame<'env, 'vm>>,
}

impl<'env, 'vm> fmt::Debug for Context<'env, 'vm> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn dump<'a>(
            m: &mut std::fmt::DebugMap,
            seen: &mut HashSet<&'a str>,
            ctx: &'a Context<'a, 'a>,
        ) -> fmt::Result {
            for frame in ctx.stack.iter().rev() {
                for (key, value) in frame.locals.iter() {
                    if !seen.contains(key) {
                        seen.insert(*key);
                        m.entry(key, value);
                    }
                }

                if let Some(ref l) = frame.current_loop {
                    if l.with_loop_var && !seen.contains(&"loop") {
                        seen.insert("loop");
                        m.entry(&"loop", &l.controller);
                    }
                }

                match frame.base {
                    FrameBase::Context(ctx) => {
                        dump(m, seen, ctx)?;
                    }
                    FrameBase::Value(ref value) => {
                        for (key, value) in value.iter_as_str_map() {
                            if !seen.contains(key) {
                                seen.insert(key);
                                m.entry(&key, &value);
                            }
                        }
                    }
                    FrameBase::None => continue,
                }
            }
            Ok(())
        }

        let mut m = f.debug_map();
        let mut seen = HashSet::new();
        dump(&mut m, &mut seen, self)?;
        m.finish()
    }
}

impl<'env, 'vm> Context<'env, 'vm> {
    /// Freezes the context.
    ///
    /// This implementation is not particularly beautiful and highly inefficient.
    /// Since it's only used for the debug support changing this is not too
    /// critical.
    #[cfg(feature = "debug")]
    fn freeze<'a>(&'a self, env: &'a Environment) -> Locals {
        let mut rv = Locals::new();

        rv.extend(env.globals.iter().map(|(k, v)| (*k, v.clone())));

        for frame in self.stack.iter().rev() {
            // look at locals first
            rv.extend(frame.locals.iter().map(|(k, v)| (*k, v.clone())));

            // if we are a loop, check if we are looking up the special loop var.
            if let Some(ref l) = frame.current_loop {
                if l.with_loop_var {
                    rv.insert("loop", Value::from_rc_object(l.controller.clone()));
                }
            }

            match frame.base {
                FrameBase::Context(ctx) => {
                    rv.extend(ctx.freeze(env));
                }
                FrameBase::Value(ref value) => {
                    rv.extend(value.iter_as_str_map());
                }
                FrameBase::None => continue,
            }
        }

        rv
    }

    /// Stores a variable in the context.
    pub fn store(&mut self, key: &'env str, value: Value) {
        self.stack
            .last_mut()
            .expect("cannot store on empty stack")
            .locals
            .insert(key, value);
    }

    /// Looks up a variable in the context.
    pub fn load(&self, env: &Environment, key: &str) -> Option<Value> {
        for frame in self.stack.iter().rev() {
            // look at locals first
            if let Some(value) = frame.locals.get(key) {
                if !value.is_undefined() {
                    return Some(value.clone());
                }
            }

            // if we are a loop, check if we are looking up the special loop var.
            if let Some(ref l) = frame.current_loop {
                if l.with_loop_var && key == "loop" {
                    return Some(Value::from_rc_object(l.controller.clone()));
                }
            }

            match frame.base {
                FrameBase::Context(ctx) => return ctx.load(env, key),
                FrameBase::Value(ref value) => {
                    let rv = value.get_attr(key);
                    if let Ok(rv) = rv {
                        if !rv.is_undefined() {
                            return Some(rv);
                        }
                    }
                    if let Some(value) = env.get_global(key) {
                        return Some(value);
                    }
                }
                FrameBase::None => continue,
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
    pub fn current_loop(&mut self) -> Option<&mut Loop> {
        self.stack
            .iter_mut()
            .rev()
            .filter_map(|x| x.current_loop.as_mut())
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
        ds.field("name", &self.name);
        ds.field("current_block", &self.current_block);
        ds.field("auto_escape", &self.auto_escape);
        ds.field("ctx", &self.ctx);
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
        value: &Value,
        args: &[Value],
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
        value: &Value,
        args: &[Value],
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
        ctx.push_frame(Frame::new(FrameBase::Value(root)));
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
                sub_context.push_frame(Frame::new(FrameBase::Context(&state.ctx)));
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

        macro_rules! super_block {
            ($capture:expr) => {
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
                    if $capture {
                        begin_capture!();
                    }
                    sub_eval!(instructions);
                    if $capture {
                        end_capture!();
                    }
                } else {
                    panic!("attempted to super unreferenced block");
                }
            };
        }

        macro_rules! recurse_loop {
            ($capture:expr) => {
                if let Some(loop_ctx) = state.ctx.current_loop() {
                    if let Some(recurse_jump_target) = loop_ctx.recurse_jump_target {
                        // the way this works is that we remember the next instruction
                        // as loop exit jump target.  Whenever a loop is pushed, it
                        // memorizes the value in `next_loop_iteration_jump` to jump
                        // to.
                        next_loop_recursion_jump = Some((pc + 1, $capture));
                        if $capture {
                            begin_capture!();
                        }
                        pc = recurse_jump_target;
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
                        "cannot recurse outside of loop"
                    ));
                }
            };
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
                    let mut v = Vec::with_capacity(*count);
                    for _ in 0..*count {
                        v.push(stack.pop());
                    }
                    v.reverse();
                    stack.push(Value(ValueRepr::Seq(RcType::new(v))));
                }
                Instruction::UnpackList(count) => {
                    let top = stack.pop();
                    let v = try_ctx!(top.as_slice().map_err(|e| {
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
                    for value in v.iter().rev() {
                        stack.push(value.clone());
                    }
                }
                Instruction::ListAppend => {
                    let item = stack.pop();
                    if let ValueRepr::Seq(mut v) = stack.pop().0 {
                        RcType::make_mut(&mut v).push(item);
                        stack.push(Value(ValueRepr::Seq(v)))
                    } else {
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
                            "cannot append to non-list"
                        ));
                    }
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
                Instruction::PushWith => {
                    state.ctx.push_frame(Frame::new(FrameBase::None));
                }
                Instruction::PopFrame => {
                    if let Some(mut loop_ctx) = state.ctx.pop_frame().current_loop {
                        if let Some((target, end_capture)) = loop_ctx.current_recursion_jump.take()
                        {
                            pc = target;
                            if end_capture {
                                end_capture!();
                            }
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
                    state.ctx.push_frame(Frame {
                        current_loop: Some(Loop {
                            iterator,
                            with_loop_var: *flags & LOOP_FLAG_WITH_LOOP_VAR != 0,
                            recurse_jump_target: if recursive { Some(pc) } else { None },
                            current_recursion_jump: next_loop_recursion_jump.take(),
                            controller: RcType::new(LoopState {
                                idx: AtomicUsize::new(!0usize),
                                len,
                                depth,
                                #[cfg(feature = "sync")]
                                last_changed_value: Mutex::default(),
                            }),
                        }),
                        ..Frame::default()
                    });
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
                        continue;
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
                Instruction::Include(ignore_missing) => {
                    let name = stack.pop();
                    let choices = if let ValueRepr::Seq(ref choices) = name.0 {
                        &choices[..]
                    } else {
                        std::slice::from_ref(&name)
                    };
                    let mut templates_tried = vec![];
                    for name in choices {
                        let name = try_ctx!(name.as_str().ok_or_else(|| {
                            Error::new(
                                ErrorKind::ImpossibleOperation,
                                "template name was not a string",
                            )
                        }));
                        let tmpl = match self.env.get_template(name) {
                            Ok(tmpl) => tmpl,
                            Err(err) => {
                                if err.kind() == ErrorKind::TemplateNotFound {
                                    templates_tried.push(name);
                                } else {
                                    bail!(err);
                                }
                                continue;
                            }
                        };
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
                        templates_tried.clear();
                        break;
                    }

                    if !templates_tried.is_empty() && !*ignore_missing {
                        if templates_tried.len() == 1 {
                            bail!(Error::new(
                                ErrorKind::TemplateNotFound,
                                format!(
                                    "tried to include non-existing template {:?}",
                                    templates_tried[0]
                                )
                            ));
                        } else {
                            bail!(Error::new(
                                ErrorKind::TemplateNotFound,
                                format!(
                                    "tried to include one of multiple templates, none of which existed {:?}",
                                    templates_tried
                                )
                            ));
                        }
                    }
                }
                Instruction::PushAutoEscape => {
                    let value = stack.pop();
                    auto_escape_stack.push(state.auto_escape);
                    state.auto_escape = match (value.as_str(), value == Value::from(true)) {
                        (Some("html"), _) => AutoEscape::Html,
                        #[cfg(feature = "json")]
                        (Some("json"), _) => AutoEscape::Json,
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
                    let top = stack.pop();
                    let args = try_ctx!(top.as_slice());
                    let value = stack.pop();
                    stack.push(try_ctx!(state.apply_filter(name, &value, args)));
                }
                Instruction::PerformTest(name) => {
                    let top = stack.pop();
                    let args = try_ctx!(top.as_slice());
                    let value = stack.pop();
                    stack.push(Value::from(
                        try_ctx!(state.perform_test(name, &value, args)),
                    ));
                }
                Instruction::CallFunction(function_name) => {
                    let top = stack.pop();
                    let args = try_ctx!(top.as_slice());
                    // super is a special function reserved for super-ing into blocks.
                    if *function_name == "super" {
                        if !args.is_empty() {
                            bail!(Error::new(
                                ErrorKind::ImpossibleOperation,
                                "super() takes no arguments",
                            ));
                        }
                        super_block!(true);
                    // loop is a special name which when called recurses the current loop.
                    } else if *function_name == "loop" {
                        if args.len() != 1 {
                            bail!(Error::new(
                                ErrorKind::ImpossibleOperation,
                                format!("loop() takes one argument, got {}", args.len())
                            ));
                        }
                        stack.push(args[0].clone());
                        recurse_loop!(true);
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
                    let top = stack.pop();
                    let args = try_ctx!(top.as_slice());
                    let obj = stack.pop();
                    stack.push(try_ctx!(obj.call_method(state, name, args)));
                }
                Instruction::CallObject => {
                    let top = stack.pop();
                    let args = try_ctx!(top.as_slice());
                    let obj = stack.pop();
                    stack.push(try_ctx!(obj.call(state, args)));
                }
                Instruction::DupTop => {
                    stack.push(stack.peek().clone());
                }
                Instruction::DiscardTop => {
                    stack.pop();
                }
                Instruction::FastSuper => {
                    super_block!(false);
                }
                Instruction::FastRecurse => {
                    recurse_loop!(false);
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
