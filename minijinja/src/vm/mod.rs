use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::environment::Environment;
use crate::error::{Error, ErrorKind};
use crate::instructions::{
    Instruction, Instructions, LOOP_FLAG_RECURSIVE, LOOP_FLAG_WITH_LOOP_VAR,
};
use crate::key::Key;
use crate::output::Output;
use crate::utils::AutoEscape;
use crate::value::{self, ops, Value, ValueRepr};
use crate::vm::context::{Context, Frame, FrameBase, Stack};
use crate::vm::forloop::{ForLoop, LoopState};

pub use crate::vm::state::State;

mod context;
mod forloop;
mod state;

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
        out: &mut Output,
    ) -> Result<Option<Value>, Error> {
        let mut ctx = Context::default();
        ctx.push_frame(Frame::new(FrameBase::Value(root)));
        let mut referenced_blocks = BTreeMap::new();
        for (&name, instr) in blocks.iter() {
            referenced_blocks.insert(name, vec![instr]);
        }
        value::with_value_optimization(|| {
            self.eval_state(&mut State {
                env: self.env,
                ctx,
                current_block: None,
                instructions,
                out,
                blocks: referenced_blocks,
            })
        })
    }

    /// This is the actual evaluation loop that works with a specific context.
    fn eval_state(&self, state: &mut State<'_, 'env, '_, '_>) -> Result<Option<Value>, Error> {
        let initial_auto_escape = state.out.auto_escape;
        let mut stack = Stack::default();
        let mut auto_escape_stack = vec![];
        let mut next_loop_recursion_jump = None;
        let mut pc = 0;

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
                    state.out.begin_capture();
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
                    write!(state.out, "{}", val)?;
                }
                Instruction::Emit => {
                    try_ctx!(self.env.format(&stack.pop(), state.out));
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
                    state.ctx.push_frame(Frame::new(FrameBase::None));
                }
                Instruction::PopFrame => {
                    if let Some(mut loop_ctx) = state.ctx.pop_frame().current_loop {
                        if let Some((target, end_capture)) = loop_ctx.current_recursion_jump.take()
                        {
                            pc = target;
                            if end_capture {
                                stack.push(state.out.end_capture());
                            }
                            continue;
                        }
                    }
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
                    if let Some(layers) = state.blocks.get(name) {
                        let instructions = layers.first().unwrap();
                        try_ctx!(self.sub_eval(state, instructions, state.blocks.clone()));
                    } else {
                        bail!(Error::new(
                            ErrorKind::ImpossibleOperation,
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
                    try_ctx!(self.perform_include(name, state, *ignore_missing));
                }
                Instruction::PushAutoEscape => {
                    let value = stack.pop();
                    auto_escape_stack.push(state.out.auto_escape);
                    state.out.auto_escape =
                        try_ctx!(self.derive_auto_escape(value, initial_auto_escape));
                }
                Instruction::PopAutoEscape => {
                    state.out.auto_escape = auto_escape_stack.pop().unwrap();
                }
                Instruction::BeginCapture => {
                    state.out.begin_capture();
                }
                Instruction::EndCapture => {
                    stack.push(state.out.end_capture());
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
                        stack.push(try_ctx!(self.perform_super(state, true)));
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
                    try_ctx!(self.perform_super(state, false));
                }
                Instruction::FastRecurse => {
                    recurse_loop!(false);
                }
            }
            pc += 1;
        }

        Ok(stack.try_pop())
    }

    fn perform_include(
        &self,
        name: Value,
        state: &mut State<'_, 'env, '_, '_>,
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
                    ErrorKind::ImpossibleOperation,
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
            let instructions = tmpl.instructions();
            let mut referenced_blocks = BTreeMap::new();
            for (&name, instr) in tmpl.blocks().iter() {
                referenced_blocks.insert(name, vec![instr]);
            }
            let original_escape = state.out.auto_escape;
            state.out.auto_escape = tmpl.initial_auto_escape();
            self.sub_eval(state, instructions, referenced_blocks)?;
            state.out.auto_escape = original_escape;
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
        state: &mut State<'_, 'env, '_, '_>,
        capture: bool,
    ) -> Result<Value, Error> {
        let mut inner_blocks = state.blocks.clone();
        let name = match state.current_block {
            Some(name) => name,
            None => {
                return Err(Error::new(
                    ErrorKind::ImpossibleOperation,
                    "cannot super outside of block",
                ));
            }
        };

        if let Some(layers) = inner_blocks.get_mut(name) {
            layers.remove(0);
            let instructions = layers.first().unwrap();
            if capture {
                state.out.begin_capture();
            }
            self.sub_eval(state, instructions, state.blocks.clone())?;
            if capture {
                Ok(state.out.end_capture())
            } else {
                Ok(Value::UNDEFINED)
            }
        } else {
            panic!("attempted to super unreferenced block");
        }
    }

    fn prepare_loop_recursion(&self, state: &mut State) -> Result<usize, Error> {
        if let Some(loop_ctx) = state.ctx.current_loop() {
            if let Some(recurse_jump_target) = loop_ctx.recurse_jump_target {
                Ok(recurse_jump_target)
            } else {
                Err(Error::new(
                    ErrorKind::ImpossibleOperation,
                    "cannot recurse outside of recursive loop",
                ))
            }
        } else {
            Err(Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot recurse outside of loop",
            ))
        }
    }

    fn load_blocks(&self, name: Value, state: &mut State<'_, 'env, '_, '_>) -> Result<(), Error> {
        let tmpl = name
            .as_str()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::ImpossibleOperation,
                    "template name was not a string",
                )
            })
            .and_then(|name| self.env.get_template(name))?;
        for (name, instr) in tmpl.blocks().iter() {
            state
                .blocks
                .entry(name)
                .or_insert_with(Vec::new)
                .push(instr);
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
                ErrorKind::ImpossibleOperation,
                "invalid value to autoescape tag",
            )),
        }
    }

    fn push_loop(
        &self,
        state: &mut State<'_, 'env, '_, '_>,
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
        let v = top.as_slice().map_err(|e| {
            Error::new(
                ErrorKind::ImpossibleOperation,
                "cannot unpack: not a sequence",
            )
            .with_source(e)
        })?;
        if v.len() != *count {
            return Err(Error::new(
                ErrorKind::ImpossibleOperation,
                format!(
                    "cannot unpack: sequence of wrong length (expected {}, got {})",
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

    fn sub_eval(
        &self,
        state: &mut State<'_, 'env, '_, '_>,
        instructions: &Instructions<'env>,
        blocks: BTreeMap<&'env str, Vec<&'_ Instructions<'env>>>,
    ) -> Result<(), Error> {
        let mut sub_context = Context::default();
        sub_context.push_frame(Frame::new(FrameBase::Context(&state.ctx)));
        self.eval_state(&mut State {
            env: self.env,
            ctx: sub_context,
            current_block: state.current_block,
            out: state.out,
            instructions,
            blocks,
        })?;
        Ok(())
    }
}

fn process_err(mut err: Error, pc: usize, state: &State) -> Error {
    if let Some(lineno) = state.instructions.get_line(pc) {
        err.set_location(state.instructions.name(), lineno);
    }
    #[cfg(feature = "debug")]
    {
        if state.env.debug() && err.debug_info.is_none() {
            err.debug_info = Some(state.make_debug_info(pc, state.instructions));
        }
    }
    err
}
