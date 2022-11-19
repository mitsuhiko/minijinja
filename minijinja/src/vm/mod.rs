use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::compiler::instructions::{
    Instruction, Instructions, LOOP_FLAG_RECURSIVE, LOOP_FLAG_WITH_LOOP_VAR, MAX_LOCALS,
};
use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::output::{CaptureMode, Output};
use crate::utils::AutoEscape;
use crate::value::{self, ops, MapType, Value, ValueMap, ValueRepr};
use crate::vm::context::{Context, Frame, LoopState, Stack};
use crate::vm::loop_object::Loop;
use crate::vm::state::BlockStack;

#[cfg(feature = "macros")]
use crate::vm::macro_object::{Macro, MacroData};

pub use crate::vm::state::State;

mod context;
mod loop_object;
#[cfg(feature = "macros")]
mod macro_object;
mod state;

// the cost of a single include against the stack limit.
#[cfg(feature = "multi-template")]
const INCLUDE_RECURSION_COST: usize = 10;

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
        let val = some!(f());
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
                    current_call: None,
                    auto_escape,
                    instructions,
                    blocks: prepare_blocks(blocks),
                    loaded_templates: BTreeSet::new(),
                    #[cfg(feature = "macros")]
                    macros: Arc::new(Vec::new()),
                },
                out,
            )
        })
    }

    /// Evaluate a macro in a state.
    #[inline(always)]
    #[cfg(feature = "macros")]
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
            let mut ctx = Context::new(Frame::new(root));
            ok!(ctx.incr_depth(state.ctx.depth()));
            self.eval_impl(
                &mut State {
                    env: self.env,
                    ctx,
                    current_block: None,
                    current_call: None,
                    auto_escape: state.auto_escape(),
                    instructions,
                    blocks: BTreeMap::default(),
                    loaded_templates: BTreeSet::new(),
                    #[cfg(feature = "macros")]
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

        // If we are extending we are holding the instructions of the target parent
        // template here.  This is used to detect multiple extends and the evaluation
        // uses these instructions when RenderParent is evaluated.
        #[cfg(feature = "multi-template")]
        let mut parent_instructions = None;

        macro_rules! recurse_loop {
            ($capture:expr) => {{
                let jump_target = ctx_ok!(self.prepare_loop_recursion(state));
                // the way this works is that we remember the next instruction
                // as loop exit jump target.  Whenever a loop is pushed, it
                // memorizes the value in `next_loop_iteration_jump` to jump
                // to.
                next_loop_recursion_jump = Some((pc + 1, $capture));
                if $capture {
                    out.begin_capture(CaptureMode::Capture);
                }
                pc = jump_target;
                continue;
            }};
        }

        while let Some(instr) = state.instructions.get(pc) {
            // if we only have two arguments that we pull from the stack, we
            // can assign them to a and b.  This slightly reduces the amount of
            // code bloat generated here.  Do the same for a potential error
            // that needs processing.
            let a;
            let b;
            let mut err;

            macro_rules! func_binop {
                ($method:ident) => {{
                    b = stack.pop();
                    a = stack.pop();
                    stack.push(ctx_ok!(ops::$method(&a, &b)));
                }};
            }

            macro_rules! op_binop {
                ($op:tt) => {{
                    b = stack.pop();
                    a = stack.pop();
                    stack.push(Value::from(a $op b));
                }};
            }

            macro_rules! bail {
                ($err:expr) => {{
                    err = $err;
                    process_err(&mut err, pc, state);
                    return Err(err);
                }};
            }

            macro_rules! ctx_ok {
                ($expr:expr) => {
                    match $expr {
                        Ok(rv) => rv,
                        Err(err) => bail!(err),
                    }
                };
            }

            match instr {
                Instruction::EmitRaw(val) => {
                    // this only produces a format error, no need to attach
                    // location information.
                    ok!(out.write_str(val).map_err(Error::from));
                }
                Instruction::Emit => {
                    ctx_ok!(self.env.format(&stack.pop(), state, out));
                }
                Instruction::StoreLocal(name) => {
                    state.ctx.store(name, stack.pop());
                }
                Instruction::Lookup(name) => {
                    stack.push(state.ctx.load(self.env, name).unwrap_or(Value::UNDEFINED));
                }
                Instruction::GetAttr(name) => {
                    a = stack.pop();
                    stack.push(ctx_ok!(a.get_attr(name)));
                }
                Instruction::GetItem => {
                    a = stack.pop();
                    b = stack.pop();
                    stack.push(ctx_ok!(b.get_item(&a)));
                }
                Instruction::Slice => {
                    let step = stack.pop();
                    let stop = stack.pop();
                    b = stack.pop();
                    a = stack.pop();
                    stack.push(ctx_ok!(ops::slice(a, b, stop, step)));
                }
                Instruction::LoadConst(value) => {
                    stack.push(value.clone());
                }
                Instruction::BuildMap(pair_count) => {
                    let mut map = ValueMap::new();
                    for _ in 0..*pair_count {
                        let value = stack.pop();
                        let key = ctx_ok!(stack.pop().try_into_key());
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
                    ctx_ok!(self.unpack_list(&mut stack, count));
                }
                Instruction::ListAppend => {
                    a = stack.pop();
                    if let ValueRepr::Seq(mut v) = stack.pop().0 {
                        Arc::make_mut(&mut v).push(a);
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
                    a = stack.pop();
                    stack.push(Value::from(!a.is_true()));
                }
                Instruction::StringConcat => {
                    a = stack.pop();
                    b = stack.pop();
                    stack.push(ops::string_concat(b, &a));
                }
                Instruction::In => {
                    a = stack.pop();
                    b = stack.pop();
                    stack.push(ctx_ok!(ops::contains(&a, &b)));
                }
                Instruction::Neg => {
                    a = stack.pop();
                    stack.push(ctx_ok!(ops::neg(&a)));
                }
                Instruction::PushWith => {
                    ctx_ok!(state.ctx.push_frame(Frame::default()));
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
                #[cfg(feature = "macros")]
                Instruction::IsUndefined => {
                    a = stack.pop();
                    stack.push(Value::from(a.is_undefined()));
                }
                Instruction::PushLoop(flags) => {
                    a = stack.pop();
                    ctx_ok!(self.push_loop(state, a, *flags, pc, next_loop_recursion_jump.take()));
                }
                Instruction::Iterate(jump_target) => {
                    let l = state.ctx.current_loop().expect("not inside a loop");
                    l.object.idx.fetch_add(1, Ordering::Relaxed);
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
                    a = stack.pop();
                    if !a.is_true() {
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
                #[cfg(feature = "multi-template")]
                Instruction::CallBlock(name) => {
                    if parent_instructions.is_none() {
                        let old_block = state.current_block;
                        state.current_block = Some(name);
                        if let Some(block_stack) = state.blocks.get(name) {
                            let old_instructions =
                                mem::replace(&mut state.instructions, block_stack.instructions());
                            ctx_ok!(state.ctx.push_frame(Frame::default()));
                            let rv = self.eval_state(state, out);
                            state.ctx.pop_frame();
                            state.instructions = old_instructions;
                            ctx_ok!(rv);
                        } else {
                            bail!(Error::new(
                                ErrorKind::InvalidOperation,
                                "tried to invoke unknown block"
                            ));
                        }
                        state.current_block = old_block;
                    }
                }
                Instruction::PushAutoEscape => {
                    a = stack.pop();
                    auto_escape_stack.push(state.auto_escape);
                    state.auto_escape = ctx_ok!(self.derive_auto_escape(a, initial_auto_escape));
                }
                Instruction::PopAutoEscape => {
                    state.auto_escape = auto_escape_stack.pop().unwrap();
                }
                Instruction::BeginCapture(mode) => {
                    out.begin_capture(*mode);
                }
                Instruction::EndCapture => {
                    stack.push(out.end_capture(state.auto_escape));
                }
                Instruction::ApplyFilter(name, arg_count, local_id) => {
                    state.current_call = Some(name);
                    let filter =
                        ctx_ok!(get_or_lookup_local(&mut loaded_filters, *local_id, || {
                            state.env.get_filter(name)
                        })
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::UnknownFilter,
                                format!("filter {} is unknown", name),
                            )
                        }));
                    let args = stack.slice_top(*arg_count);
                    a = ctx_ok!(filter.apply_to(state, args));
                    stack.drop_top(*arg_count);
                    stack.push(a);
                    state.current_call = Some(name);
                }
                Instruction::PerformTest(name, arg_count, local_id) => {
                    state.current_call = Some(name);
                    let test = ctx_ok!(get_or_lookup_local(&mut loaded_tests, *local_id, || {
                        state.env.get_test(name)
                    })
                    .ok_or_else(|| {
                        Error::new(ErrorKind::UnknownTest, format!("test {} is unknown", name))
                    }));
                    let args = stack.slice_top(*arg_count);
                    let rv = ctx_ok!(test.perform(state, args));
                    stack.drop_top(*arg_count);
                    stack.push(Value::from(rv));
                    state.current_call = None;
                }
                Instruction::CallFunction(name, arg_count) => {
                    state.current_call = Some(name);

                    // super is a special function reserved for super-ing into blocks.
                    if *name == "super" {
                        if *arg_count != 0 {
                            bail!(Error::new(
                                ErrorKind::InvalidOperation,
                                "super() takes no arguments",
                            ));
                        }
                        stack.push(ctx_ok!(self.perform_super(state, out, true)));
                    // loop is a special name which when called recurses the current loop.
                    } else if *name == "loop" {
                        if *arg_count != 1 {
                            bail!(Error::new(
                                ErrorKind::InvalidOperation,
                                format!("loop() takes one argument, got {}", *arg_count)
                            ));
                        }
                        // leave the one argument on the stack for the recursion
                        recurse_loop!(true);
                    } else if let Some(func) = state.ctx.load(self.env, name) {
                        let args = stack.slice_top(*arg_count);
                        a = ctx_ok!(func.call(state, args));
                        stack.drop_top(*arg_count);
                        stack.push(a);
                    } else {
                        bail!(Error::new(
                            ErrorKind::UnknownFunction,
                            format!("{} is unknown", name),
                        ));
                    }

                    state.current_call = None;
                }
                Instruction::CallMethod(name, arg_count) => {
                    state.current_call = Some(name);
                    let args = stack.slice_top(*arg_count);
                    a = ctx_ok!(args[0].call_method(state, name, &args[1..]));
                    stack.drop_top(*arg_count);
                    stack.push(a);
                    state.current_call = None;
                }
                Instruction::CallObject(arg_count) => {
                    let args = stack.slice_top(*arg_count);
                    a = ctx_ok!(args[0].call(state, &args[1..]));
                    stack.drop_top(*arg_count);
                    stack.push(a);
                }
                Instruction::DupTop => {
                    stack.push(stack.peek().clone());
                }
                Instruction::DiscardTop => {
                    stack.pop();
                }
                Instruction::FastSuper => {
                    // Note that we don't store 'current_call' here since it
                    // would only be visible (and unused) internally.
                    ctx_ok!(self.perform_super(state, out, false));
                }
                Instruction::FastRecurse => {
                    // Note that we don't store 'current_call' here since it
                    // would only be visible (and unused) internally.
                    recurse_loop!(false);
                }
                // Explanation on the behavior of `LoadBlocks` and `RenderParent`.
                // MiniJinja inherits the behavior from Jinja2 where extending
                // loads the blocks (`LoadBlocks`) and the rest of the template
                // keeps executing but with output disabled, only at the end the
                // parent template is then invoked (`RenderParent`).  This has the
                // effect that you can still set variables or declare macros and
                // that they become visible in the blocks.
                //
                // This behavior has a few downsides.  First of all what happens
                // in the parent template overrides what happens in the child.
                // For instance if you declare a macro named `foo` after `{%
                // extends %}` and then a variable with that named is also set
                // in the parent template, then you won't be able to call that
                // macro in the body.
                //
                // The reason for this is that blocks unlike macros do not have
                // closures in Jinja2/MiniJinja.
                //
                // However for the common case this is convenient because it
                // lets you put some imports there and for as long as you do not
                // create name clashes this works fine.
                #[cfg(feature = "multi-template")]
                Instruction::LoadBlocks => {
                    a = stack.pop();
                    if parent_instructions.is_some() {
                        bail!(Error::new(
                            ErrorKind::InvalidOperation,
                            "tried to extend a second time in a template"
                        ));
                    }
                    parent_instructions = Some(ctx_ok!(self.load_blocks(a, state)));
                    out.begin_capture(CaptureMode::Discard);
                }
                #[cfg(feature = "multi-template")]
                Instruction::RenderParent => {
                    out.end_capture(AutoEscape::None);
                    state.instructions = parent_instructions.take().unwrap();

                    // then replace the instructions and set the pc to 0 again.
                    // this effectively means that the template engine will now
                    // execute the extended template's code instead.  From this
                    // there is no way back.
                    pc = 0;
                    continue;
                }
                #[cfg(feature = "multi-template")]
                Instruction::Include(ignore_missing) => {
                    a = stack.pop();
                    ctx_ok!(self.perform_include(a, state, out, *ignore_missing));
                }
                #[cfg(feature = "multi-template")]
                Instruction::ExportLocals => {
                    let locals = state.ctx.current_locals();
                    let mut module = ValueMap::new();
                    for (key, value) in locals.iter() {
                        module.insert((*key).into(), value.clone());
                    }
                    stack.push(Value(ValueRepr::Map(module.into(), MapType::Normal)));
                }
                #[cfg(feature = "macros")]
                Instruction::BuildMacro(name, offset, self_reference) => {
                    self.build_macro(&mut stack, state, *offset, name, *self_reference);
                }
                #[cfg(feature = "macros")]
                Instruction::Return => break,
            }
            pc += 1;
        }

        Ok(stack.try_pop())
    }

    #[cfg(feature = "multi-template")]
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
            let name = ok!(name.as_str().ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    "template name was not a string",
                )
            }));
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
            ok!(state.ctx.incr_depth(INCLUDE_RECURSION_COST));
            let rv = self.eval_state(state, out);
            state.ctx.decr_depth(INCLUDE_RECURSION_COST);
            state.auto_escape = old_escape;
            state.instructions = old_instructions;
            state.blocks = old_blocks;
            ok!(rv.map_err(|err| {
                Error::new(
                    ErrorKind::BadInclude,
                    format!("error in \"{}\"", tmpl.name()),
                )
                .with_source(err)
            }));
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
        let name = ok!(state.current_block.ok_or_else(|| {
            Error::new(ErrorKind::InvalidOperation, "cannot super outside of block")
        }));

        let block_stack = state.blocks.get_mut(name).unwrap();
        if !block_stack.push() {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "no parent block exists",
            ));
        }

        if capture {
            out.begin_capture(CaptureMode::Capture);
        }

        let old_instructions = mem::replace(&mut state.instructions, block_stack.instructions());
        ok!(state.ctx.push_frame(Frame::default()));
        let rv = self.eval_state(state, out);
        state.ctx.pop_frame();
        state.instructions = old_instructions;
        state.blocks.get_mut(name).unwrap().pop();

        ok!(rv.map_err(|err| {
            Error::new(ErrorKind::EvalBlock, "error in super block").with_source(err)
        }));
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

    #[cfg(feature = "multi-template")]
    fn load_blocks(
        &self,
        name: Value,
        state: &mut State<'_, 'env>,
    ) -> Result<&'env Instructions<'env>, Error> {
        let name = match name.as_str() {
            Some(name) => name,
            None => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "template name was not a string",
                ))
            }
        };
        if state.loaded_templates.contains(&name) {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "cycle in template inheritance. {:?} was referenced more than once",
                    name
                ),
            ));
        }
        let tmpl = ok!(self.env.get_template(name));
        state.loaded_templates.insert(tmpl.instructions().name());
        for (name, instr) in tmpl.blocks().iter() {
            state
                .blocks
                .entry(name)
                .or_insert_with(BlockStack::default)
                .append_instructions(instr);
        }
        Ok(tmpl.instructions())
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
        current_recursion_jump: Option<(usize, bool)>,
    ) -> Result<(), Error> {
        let iterator = ok!(iterable.try_iter_owned());
        let len = iterator.len();
        let depth = state
            .ctx
            .current_loop()
            .filter(|x| x.recurse_jump_target.is_some())
            .map_or(0, |x| x.object.depth + 1);
        let recursive = flags & LOOP_FLAG_RECURSIVE != 0;
        let with_loop_var = flags & LOOP_FLAG_WITH_LOOP_VAR != 0;
        ok!(state.ctx.push_frame(Frame {
            current_loop: Some(LoopState {
                iterator,
                with_loop_var,
                recurse_jump_target: if recursive { Some(pc) } else { None },
                current_recursion_jump,
                object: Arc::new(Loop {
                    idx: AtomicUsize::new(!0usize),
                    len,
                    depth,
                    last_changed_value: Mutex::default(),
                }),
            }),
            ..Frame::default()
        }));
        Ok(())
    }

    fn unpack_list(&self, stack: &mut Stack, count: &usize) -> Result<(), Error> {
        let top = stack.pop();
        let v =
            ok!(top
                .as_slice()
                .map_err(|e| Error::new(ErrorKind::CannotUnpack, "not a sequence").with_source(e)));
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

    #[cfg(feature = "macros")]
    fn build_macro(
        &self,
        stack: &mut Stack,
        state: &mut State,
        offset: usize,
        name: &str,
        self_reference: bool,
    ) {
        let arg_spec = match stack.pop().0 {
            ValueRepr::Seq(args) => args
                .iter()
                .map(|value| match &value.0 {
                    ValueRepr::String(arg, _) => arg.clone(),
                    _ => unreachable!(),
                })
                .collect(),
            _ => unreachable!(),
        };
        let closure = stack.pop();
        let macro_ref_id = state.macros.len();
        Arc::make_mut(&mut state.macros).push((state.instructions, offset));
        stack.push(Value::from_object(Macro {
            data: Arc::new(MacroData {
                name: Arc::new(name.to_string()),
                arg_spec,
                macro_ref_id,
                closure,
                self_reference,
            }),
        }));
    }
}

#[inline(never)]
#[cold]
fn process_err(err: &mut Error, pc: usize, state: &State) {
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
}
