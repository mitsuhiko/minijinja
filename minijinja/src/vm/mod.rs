use std::collections::BTreeMap;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::compiler::instructions::{
    Instruction, Instructions, LOOP_FLAG_RECURSIVE, LOOP_FLAG_WITH_LOOP_VAR, MAX_LOCALS,
};
use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::key::Key;
use crate::output::Output;
use crate::utils::AutoEscape;
use crate::value::{self, ops, MapType, Value, ValueMap, ValueRepr};
use crate::vm::context::{Context, Frame, Stack};
use crate::vm::loop_object::{ForLoop, LoopState};
use crate::vm::macro_object::Macro;
use crate::vm::state::BlockStack;

pub use crate::vm::state::State;

mod context;
mod loop_object;
mod macro_object;
mod state;

/// Helps to evaluate something.
#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub struct Vm<'env> {
    env: &'env Environment<'env>,
}

fn prepare_blocks<'env, 'vm>(
    blocks: &'vm BTreeMap<&'env str, Instructions<'env>>,
) -> BTreeMap<&'env str, BlockStack<'vm, 'env>> {
    blocks
        .iter()
        .map(|(name, instr)| (*name, BlockStack::new(instr)))
        .collect()
}

#[inline(always)]
fn get_or_lookup_local<T, F>(vec: &mut [Option<T>], local_id: u8, f: F) -> Option<T>
where
    T: Copy,
    F: FnOnce() -> Option<T>,
{
    if local_id == !0 {
        f()
    } else if let Some(Some(rv)) = vec.get(local_id as usize) {
        Some(*rv)
    } else {
        let val = f()?;
        vec[local_id as usize] = Some(val);
        Some(val)
    }
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
        out: &mut Output,
        auto_escape: AutoEscape,
    ) -> Result<Option<Value>, Error> {
        value::with_value_optimization(|| {
            self.eval_state(
                &mut State {
                    env: self.env,
                    ctx: Context::new(Frame::new(root)),
                    current_block: None,
                    instructions,
                    auto_escape,
                    blocks: prepare_blocks(blocks),
                    macros: Arc::new(Vec::new()),
                },
                out,
            )
        })
    }

    /// Evaluate a macro in a state.
    #[inline(always)]
    pub fn eval_macro(
        &self,
        instructions: &Instructions<'env>,
        pc: usize,
        root: Value,
        out: &mut Output,
        state: &State,
        args: Vec<Value>,
    ) -> Result<Option<Value>, Error> {
        value::with_value_optimization(|| {
            self.eval_impl(
                &mut State {
                    env: self.env,
                    ctx: Context::new(Frame::new(root)),
                    current_block: None,
                    instructions,
                    auto_escape: state.auto_escape(),
                    blocks: BTreeMap::default(),
                    macros: state.macros.clone(),
                },
                out,
                Stack::from(args),
                pc,
            )
        })
    }

    /// This is the actual evaluation loop that works with a specific context.
    #[inline(always)]
    fn eval_state(
        &self,
        state: &mut State<'_, 'env>,
        out: &mut Output,
    ) -> Result<Option<Value>, Error> {
        self.eval_impl(state, out, Stack::default(), 0)
    }

    fn eval_impl(
        &self,
        state: &mut State<'_, 'env>,
        out: &mut Output,
        mut stack: Stack,
        mut pc: usize,
    ) -> Result<Option<Value>, Error> {
        let initial_auto_escape = state.auto_escape;
        let mut auto_escape_stack = vec![];
        let mut next_loop_recursion_jump = None;
        let mut loaded_filters = [None; MAX_LOCALS];
        let mut loaded_tests = [None; MAX_LOCALS];

        macro_rules! bail {
            ($err:expr) => {{
                return Err(process_err($err, pc, state));
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
                stack.push(try_ctx!(ops::$method(&a, &b)));
            }};
        }

        macro_rules! op_binop {
            ($op:tt) => {{
                let b = stack.pop();
                let a = stack.pop();
                stack.push(Value::from(a $op b));
            }};
        }

        macro_rules! recurse_loop {
            ($capture:expr) => {{
                let jump_target = try_ctx!(self.prepare_loop_recursion(state));
                // the way this works is that we remember the next instruction
                // as loop exit jump target.  Whenever a loop is pushed, it
                // memorizes the value in `next_loop_iteration_jump` to jump
                // to.
                next_loop_recursion_jump = Some((pc + 1, $capture));
                if $capture {
                    out.begin_capture();
                }
                pc = jump_target;
                continue;
            }};
        }

        while let Some(instr) = state.instructions.get(pc) {
            match instr {
                Instruction::EmitRaw(val) => {
                    // this only produces a format error, no need to attach
                    // location information.
                    out.write_str(val)?;
                }
                Instruction::Emit => {
                    try_ctx!(self.env.format(&stack.pop(), state, out));
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
                Instruction::Slice => {
                    let step = stack.pop();
                    let stop = stack.pop();
                    let start = stack.pop();
                    let value = stack.pop();
                    stack.push(try_ctx!(ops::slice(value, start, stop, step)));
                }
                Instruction::LoadConst(value) => {
                    stack.push(value.clone());
                }
                Instruction::BuildMap(pair_count) => {
                    let mut map = ValueMap::new();
                    for _ in 0..*pair_count {
                        let value = stack.pop();
                        let key = try_ctx!(stack.pop().try_into_key());
                        map.insert(key, value);
                    }
                    stack.push(Value(ValueRepr::Map(map.into(), MapType::Normal)))
                }
                Instruction::BuildKwargs(pair_count) => {
                    let mut map = ValueMap::new();
                    for _ in 0..*pair_count {
                        let value = stack.pop();
                        let key = stack.pop().try_into_key().unwrap();
                        map.insert(key, value);
                    }
                    stack.push(Value(ValueRepr::Map(map.into(), MapType::Kwargs)))
                }
                Instruction::BuildList(count) => {
                    let mut v = Vec::with_capacity(*count);
                    for _ in 0..*count {
                        v.push(stack.pop());
                    }
                    v.reverse();
                    stack.push(Value(ValueRepr::Seq(Arc::new(v))));
                }
                Instruction::UnpackList(count) => {
                    try_ctx!(self.unpack_list(&mut stack, count));
                }
                Instruction::ListAppend => {
                    let item = stack.pop();
                    if let ValueRepr::Seq(mut v) = stack.pop().0 {
                        Arc::make_mut(&mut v).push(item);
                        stack.push(Value(ValueRepr::Seq(v)))
                    } else {
                        bail!(Error::new(
                            ErrorKind::InvalidOperation,
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
                    stack.push(ops::string_concat(b, &a));
                }
                Instruction::In => {
                    let container = stack.pop();
                    let value = stack.pop();
                    stack.push(try_ctx!(ops::contains(&container, &value)));
                }
                Instruction::Neg => {
                    let a = stack.pop();
                    stack.push(try_ctx!(ops::neg(&a)));
                }
                Instruction::PushWith => {
                    state.ctx.push_frame(Frame::default());
                }
                Instruction::PopFrame => {
                    if let Some(mut loop_ctx) = state.ctx.pop_frame().current_loop {
                        if let Some((target, end_capture)) = loop_ctx.current_recursion_jump.take()
                        {
                            pc = target;
                            if end_capture {
                                stack.push(out.end_capture(state.auto_escape));
                            }
                            continue;
                        }
                    }
                }
                Instruction::IsUndefined => {
                    let value = stack.pop();
                    stack.push(Value::from(value.is_undefined()));
                }
                Instruction::PushLoop(flags) => {
                    let iterable = stack.pop();
                    try_ctx!(self.push_loop(
                        state,
                        iterable,
                        *flags,
                        pc,
                        next_loop_recursion_jump.take()
                    ));
                }
                Instruction::Iterate(jump_target) => {
                    let l = state.ctx.current_loop().expect("not inside a loop");
                    l.state.idx.fetch_add(1, Ordering::Relaxed);
                    match l.iterator.next() {
                        Some(item) => stack.push(item),
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
                    let old_block = state.current_block;
                    state.current_block = Some(name);
                    if let Some(block_stack) = state.blocks.get(name) {
                        let old_instructions =
                            mem::replace(&mut state.instructions, block_stack.instructions());
                        state.ctx.push_frame(Frame::default());
                        let rv = self.eval_state(state, out);
                        state.ctx.pop_frame();
                        state.instructions = old_instructions;
                        try_ctx!(rv);
                    } else {
                        bail!(Error::new(
                            ErrorKind::InvalidOperation,
                            "tried to invoke unknown block"
                        ));
                    }
                    state.current_block = old_block;
                }
                Instruction::LoadBlocks => {
                    let name = stack.pop();
                    try_ctx!(self.load_blocks(name, state));

                    // then replace the instructions and set the pc to 0 again.
                    // this effectively means that the template engine will now
                    // execute the extended template's code instead.  From this
                    // there is no way back.
                    pc = 0;
                    continue;
                }
                Instruction::Include(ignore_missing) => {
                    let name = stack.pop();
                    try_ctx!(self.perform_include(name, state, out, *ignore_missing));
                }
                Instruction::PushAutoEscape => {
                    let value = stack.pop();
                    auto_escape_stack.push(state.auto_escape);
                    state.auto_escape =
                        try_ctx!(self.derive_auto_escape(value, initial_auto_escape));
                }
                Instruction::PopAutoEscape => {
                    state.auto_escape = auto_escape_stack.pop().unwrap();
                }
                Instruction::BeginCapture => {
                    out.begin_capture();
                }
                Instruction::EndCapture => {
                    stack.push(out.end_capture(state.auto_escape));
                }
                Instruction::ApplyFilter(name, arg_count, local_id) => {
                    let filter =
                        try_ctx!(get_or_lookup_local(&mut loaded_filters, *local_id, || {
                            state.env.get_filter(name)
                        })
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::UnknownFilter,
                                format!("filter {} is unknown", name),
                            )
                        }));
                    let args = stack.slice_top(*arg_count);
                    let rv = try_ctx!(filter.apply_to(state, args));
                    stack.drop_top(*arg_count);
                    stack.push(rv);
                }
                Instruction::PerformTest(name, arg_count, local_id) => {
                    let test = try_ctx!(get_or_lookup_local(&mut loaded_tests, *local_id, || {
                        state.env.get_test(name)
                    })
                    .ok_or_else(|| {
                        Error::new(ErrorKind::UnknownTest, format!("test {} is unknown", name))
                    }));
                    let args = stack.slice_top(*arg_count);
                    let rv = try_ctx!(test.perform(state, args));
                    stack.drop_top(*arg_count);
                    stack.push(Value::from(rv));
                }
                Instruction::CallFunction(function_name, arg_count) => {
                    // super is a special function reserved for super-ing into blocks.
                    if *function_name == "super" {
                        if *arg_count != 0 {
                            bail!(Error::new(
                                ErrorKind::InvalidOperation,
                                "super() takes no arguments",
                            ));
                        }
                        stack.push(try_ctx!(self.perform_super(state, out, true)));
                    // loop is a special name which when called recurses the current loop.
                    } else if *function_name == "loop" {
                        if *arg_count != 1 {
                            bail!(Error::new(
                                ErrorKind::InvalidOperation,
                                format!("loop() takes one argument, got {}", *arg_count)
                            ));
                        }
                        // leave the one argument on the stack for the recursion
                        recurse_loop!(true);
                    } else if let Some(func) = state.ctx.load(self.env, function_name) {
                        let args = stack.slice_top(*arg_count);
                        let rv = try_ctx!(func.call(state, args));
                        stack.drop_top(*arg_count);
                        stack.push(rv);
                    } else {
                        bail!(Error::new(
                            ErrorKind::UnknownFunction,
                            format!("{} is unknown", function_name),
                        ));
                    }
                }
                Instruction::CallMethod(name, arg_count) => {
                    let args = stack.slice_top(*arg_count);
                    let rv = try_ctx!(args[0].call_method(state, name, &args[1..]));
                    stack.drop_top(*arg_count);
                    stack.push(rv);
                }
                Instruction::CallObject(arg_count) => {
                    let args = stack.slice_top(*arg_count);
                    let rv = try_ctx!(args[0].call(state, &args[1..]));
                    stack.drop_top(*arg_count);
                    stack.push(rv);
                }
                Instruction::DupTop => {
                    stack.push(stack.peek().clone());
                }
                Instruction::DiscardTop => {
                    stack.pop();
                }
                Instruction::FastSuper => {
                    try_ctx!(self.perform_super(state, out, false));
                }
                Instruction::FastRecurse => {
                    recurse_loop!(false);
                }
                Instruction::BuildMacro(name, offset) => {
                    self.build_macro(&mut stack, state, offset, name);
                }
                Instruction::ExportLocals => {
                    let locals = state.ctx.current_locals();
                    let mut module = ValueMap::new();
                    for (key, value) in locals.iter() {
                        module.insert(Key::make_string_key(key), value.clone());
                    }
                    stack.push(Value(ValueRepr::Map(module.into(), MapType::Normal)));
                }
                Instruction::Return => break,
            }
            pc += 1;
        }

        Ok(stack.try_pop())
    }

    fn perform_include(
        &self,
        name: Value,
        state: &mut State<'_, 'env>,
        out: &mut Output,
        ignore_missing: bool,
    ) -> Result<(), Error> {
        let choices = if let ValueRepr::Seq(ref choices) = name.0 {
            &choices[..]
        } else {
            std::slice::from_ref(&name)
        };
        let mut templates_tried = vec![];
        for name in choices {
            let name = name.as_str().ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    "template name was not a string",
                )
            })?;
            let tmpl = match self.env.get_template(name) {
                Ok(tmpl) => tmpl,
                Err(err) => {
                    if err.kind() == ErrorKind::TemplateNotFound {
                        templates_tried.push(name);
                    } else {
                        return Err(err);
                    }
                    continue;
                }
            };
            let old_escape = mem::replace(&mut state.auto_escape, tmpl.initial_auto_escape());
            let old_instructions = mem::replace(&mut state.instructions, tmpl.instructions());
            let old_blocks = mem::replace(&mut state.blocks, prepare_blocks(tmpl.blocks()));
            let rv = self.eval_state(state, out);
            state.auto_escape = old_escape;
            state.instructions = old_instructions;
            state.blocks = old_blocks;
            rv.map_err(|err| {
                Error::new(
                    ErrorKind::BadInclude,
                    format!("error in \"{}\"", tmpl.name()),
                )
                .with_source(err)
            })?;
            return Ok(());
        }
        if !templates_tried.is_empty() && !ignore_missing {
            Err(Error::new(
                ErrorKind::TemplateNotFound,
                if templates_tried.len() == 1 {
                    format!(
                        "tried to include non-existing template {:?}",
                        templates_tried[0]
                    )
                } else {
                    format!(
                        "tried to include one of multiple templates, none of which existed {:?}",
                        templates_tried
                    )
                },
            ))
        } else {
            Ok(())
        }
    }

    fn perform_super(
        &self,
        state: &mut State<'_, 'env>,
        out: &mut Output,
        capture: bool,
    ) -> Result<Value, Error> {
        let name = state.current_block.ok_or_else(|| {
            Error::new(ErrorKind::InvalidOperation, "cannot super outside of block")
        })?;

        let block_stack = state.blocks.get_mut(name).unwrap();
        if !block_stack.push() {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "no parent block exists",
            ));
        }

        if capture {
            out.begin_capture();
        }

        let old_instructions = mem::replace(&mut state.instructions, block_stack.instructions());
        let rv = self.eval_state(state, out);
        state.instructions = old_instructions;
        state.blocks.get_mut(name).unwrap().pop();

        rv.map_err(|err| {
            Error::new(ErrorKind::EvalBlock, "error in super block").with_source(err)
        })?;
        if capture {
            Ok(out.end_capture(state.auto_escape))
        } else {
            Ok(Value::UNDEFINED)
        }
    }

    fn prepare_loop_recursion(&self, state: &mut State) -> Result<usize, Error> {
        if let Some(loop_ctx) = state.ctx.current_loop() {
            if let Some(recurse_jump_target) = loop_ctx.recurse_jump_target {
                Ok(recurse_jump_target)
            } else {
                Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "cannot recurse outside of recursive loop",
                ))
            }
        } else {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                "cannot recurse outside of loop",
            ))
        }
    }

    fn load_blocks(&self, name: Value, state: &mut State<'_, 'env>) -> Result<(), Error> {
        let tmpl = name
            .as_str()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    "template name was not a string",
                )
            })
            .and_then(|name| self.env.get_template(name))?;
        for (name, instr) in tmpl.blocks().iter() {
            state
                .blocks
                .entry(name)
                .or_insert_with(BlockStack::default)
                .append_instructions(instr);
        }
        state.instructions = tmpl.instructions();
        Ok(())
    }

    fn derive_auto_escape(
        &self,
        value: Value,
        initial_auto_escape: AutoEscape,
    ) -> Result<AutoEscape, Error> {
        match (value.as_str(), value == Value::from(true)) {
            (Some("html"), _) => Ok(AutoEscape::Html),
            #[cfg(feature = "json")]
            (Some("json"), _) => Ok(AutoEscape::Json),
            (Some("none"), _) | (None, false) => Ok(AutoEscape::None),
            (None, true) => Ok(if matches!(initial_auto_escape, AutoEscape::None) {
                AutoEscape::Html
            } else {
                initial_auto_escape
            }),
            _ => Err(Error::new(
                ErrorKind::InvalidOperation,
                "invalid value to autoescape tag",
            )),
        }
    }

    fn push_loop(
        &self,
        state: &mut State<'_, 'env>,
        iterable: Value,
        flags: u8,
        pc: usize,
        next_loop_recursion_jump: Option<(usize, bool)>,
    ) -> Result<(), Error> {
        let iterator = iterable.try_iter()?;
        let len = iterator.len();
        let depth = state
            .ctx
            .current_loop()
            .filter(|x| x.recurse_jump_target.is_some())
            .map_or(0, |x| x.state.depth + 1);
        let recursive = flags & LOOP_FLAG_RECURSIVE != 0;
        state.ctx.push_frame(Frame {
            current_loop: Some(ForLoop {
                iterator,
                with_loop_var: flags & LOOP_FLAG_WITH_LOOP_VAR != 0,
                recurse_jump_target: if recursive { Some(pc) } else { None },
                current_recursion_jump: next_loop_recursion_jump,
                state: Arc::new(LoopState {
                    idx: AtomicUsize::new(!0usize),
                    len,
                    depth,
                    last_changed_value: Mutex::default(),
                }),
            }),
            ..Frame::default()
        });
        Ok(())
    }

    fn unpack_list(&self, stack: &mut Stack, count: &usize) -> Result<(), Error> {
        let top = stack.pop();
        let v = top
            .as_slice()
            .map_err(|e| Error::new(ErrorKind::CannotUnpack, "not a sequence").with_source(e))?;
        if v.len() != *count {
            return Err(Error::new(
                ErrorKind::CannotUnpack,
                format!(
                    "sequence of wrong length (expected {}, got {})",
                    *count,
                    v.len()
                ),
            ));
        }
        for value in v.iter().rev() {
            stack.push(value.clone());
        }
        Ok(())
    }

    fn build_macro(&self, stack: &mut Stack, state: &mut State, offset: &usize, name: &&str) {
        let arg_spec = match stack.pop().0 {
            ValueRepr::Seq(args) => args
                .iter()
                .map(|value| match &value.0 {
                    ValueRepr::String(arg) => arg.clone(),
                    _ => unreachable!(),
                })
                .collect(),
            _ => unreachable!(),
        };
        let closure = stack.pop();
        let macro_ref_id = state.macros.len();
        Arc::make_mut(&mut state.macros).push((state.instructions, *offset as usize));
        stack.push(Value::from_object(Macro {
            name: Arc::new(name.to_string()),
            arg_spec,
            macro_ref_id,
            closure,
        }));
    }
}

fn process_err(mut err: Error, pc: usize, state: &State) -> Error {
    // only attach line information if the error does not have line info yet.
    if err.line().is_none() {
        if let Some(span) = state.instructions.get_span(pc) {
            err.set_filename_and_span(state.instructions.name(), span);
        } else if let Some(lineno) = state.instructions.get_line(pc) {
            err.set_filename_and_line(state.instructions.name(), lineno);
        }
    }
    // only attach debug info if we don't have one yet and we are in debug mode.
    #[cfg(feature = "debug")]
    {
        if state.env.debug() && err.debug_info().is_none() {
            err.attach_debug_info(state.make_debug_info(pc, state.instructions));
        }
    }
    err
}
