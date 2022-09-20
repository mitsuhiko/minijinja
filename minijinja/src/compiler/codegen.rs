use std::collections::BTreeMap;

use crate::compiler::ast;
use crate::compiler::instructions::{
    Instruction, Instructions, LOOP_FLAG_RECURSIVE, LOOP_FLAG_WITH_LOOP_VAR,
};
use crate::compiler::tokens::Span;
use crate::error::Error;
use crate::value::Value;

#[cfg(test)]
use similar_asserts::assert_eq;

/// Represents an open block of code that does not yet have updated
/// jump targets.
#[cfg_attr(feature = "internal_debug", derive(Debug))]
enum PendingBlock {
    Branch(usize),
    Loop(usize),
    ScBool(Vec<usize>),
}

/// Provides a convenient interface to creating instructions for the VM.
#[cfg_attr(feature = "internal_debug", derive(Debug))]
pub struct CodeGenerator<'source> {
    instructions: Instructions<'source>,
    blocks: BTreeMap<&'source str, Instructions<'source>>,
    pending_block: Vec<PendingBlock>,
    current_line: usize,
    span_stack: Vec<Span>,
}

impl<'source> CodeGenerator<'source> {
    /// Creates a new code generator.
    pub fn new(file: &'source str, source: &'source str) -> CodeGenerator<'source> {
        CodeGenerator {
            instructions: Instructions::new(file, source),
            blocks: BTreeMap::new(),
            pending_block: Vec::new(),
            current_line: 0,
            span_stack: Vec::new(),
        }
    }

    /// Sets the current location's line.
    pub fn set_line(&mut self, lineno: usize) {
        self.current_line = lineno;
    }

    /// Sets line from span.
    pub fn set_line_from_span(&mut self, span: Span) {
        self.set_line(span.start_line);
    }

    /// Pushes a span to the stack
    pub fn push_span(&mut self, span: Span) {
        self.span_stack.push(span);
        self.set_line_from_span(span);
    }

    /// Pops a span from the stack.
    pub fn pop_span(&mut self) {
        self.span_stack.pop();
    }

    /// Add a simple instruction with the current location.
    pub fn add(&mut self, instr: Instruction<'source>) -> usize {
        if let Some(span) = self.span_stack.last() {
            if span.start_line == self.current_line {
                return self.instructions.add_with_span(instr, *span);
            }
        }
        self.instructions.add_with_line(instr, self.current_line)
    }

    /// Add a simple instruction with other location.
    pub fn add_with_span(&mut self, instr: Instruction<'source>, span: Span) -> usize {
        self.instructions.add_with_span(instr, span)
    }

    /// Returns the next instruction index.
    pub fn next_instruction(&self) -> usize {
        self.instructions.len()
    }

    /// Creats a sub generator.
    fn new_subgenerator(&self) -> CodeGenerator<'source> {
        let mut sub = CodeGenerator::new(self.instructions.name(), self.instructions.source());
        sub.current_line = self.current_line;
        sub.span_stack = self.span_stack.last().copied().into_iter().collect();
        sub
    }

    /// Finishes a sub generator and syncs it back.
    fn finish_subgenerator(&mut self, sub: CodeGenerator<'source>) -> Instructions<'source> {
        self.current_line = sub.current_line;
        let (instructions, blocks) = sub.finish();
        self.blocks.extend(blocks.into_iter());
        instructions
    }

    /// Starts a for loop
    pub fn start_for_loop(&mut self, with_loop_var: bool, recursive: bool) {
        let mut flags = 0;
        if with_loop_var {
            flags |= LOOP_FLAG_WITH_LOOP_VAR;
        }
        if recursive {
            flags |= LOOP_FLAG_RECURSIVE;
        }
        self.add(Instruction::PushLoop(flags));
        let iter_instr = self.add(Instruction::Iterate(!0));
        self.pending_block.push(PendingBlock::Loop(iter_instr));
    }

    /// Ends the open for loop
    pub fn end_for_loop(&mut self, push_did_iterate: bool) {
        match self.pending_block.pop() {
            Some(PendingBlock::Loop(iter_instr)) => {
                self.add(Instruction::Jump(iter_instr));
                let loop_end = self.next_instruction();
                if push_did_iterate {
                    self.add(Instruction::Lookup("loop"));
                    self.add(Instruction::GetAttr("index0"));
                    self.add(Instruction::LoadConst(Value::from(0)));
                    self.add(Instruction::Eq);
                };
                self.add(Instruction::PopFrame);
                if let Some(Instruction::Iterate(ref mut jump_target)) =
                    self.instructions.get_mut(iter_instr)
                {
                    *jump_target = loop_end;
                } else {
                    panic!("did not find iteration instruction");
                }
            }
            _ => panic!("not inside a loop"),
        }
    }

    /// Begins an if conditional
    pub fn start_if(&mut self) {
        let jump_instr = self.add(Instruction::JumpIfFalse(!0));
        self.pending_block.push(PendingBlock::Branch(jump_instr));
    }

    /// Begins an else conditional
    pub fn start_else(&mut self) {
        let jump_instr = self.add(Instruction::Jump(!0));
        self.end_condition(jump_instr + 1);
        self.pending_block.push(PendingBlock::Branch(jump_instr));
    }

    /// Closes the current if block.
    pub fn end_if(&mut self) {
        self.end_condition(self.next_instruction());
    }

    /// Starts a short cirquited bool block.
    pub fn start_sc_bool(&mut self) {
        self.pending_block.push(PendingBlock::ScBool(vec![]));
    }

    /// Emits a short circuited bool operator.
    pub fn sc_bool(&mut self, and: bool) {
        if let Some(PendingBlock::ScBool(ref mut instructions)) = self.pending_block.last_mut() {
            instructions.push(self.instructions.add(if and {
                Instruction::JumpIfFalseOrPop(!0)
            } else {
                Instruction::JumpIfTrueOrPop(!0)
            }));
        } else {
            panic!("tried to emit sc_bool from outside of sc_bool block");
        }
    }

    /// Ends a short circuited bool block.
    pub fn end_sc_bool(&mut self) {
        let end = self.next_instruction();
        if let Some(PendingBlock::ScBool(instructions)) = self.pending_block.pop() {
            for instr in instructions {
                match self.instructions.get_mut(instr) {
                    Some(Instruction::JumpIfFalseOrPop(ref mut target))
                    | Some(Instruction::JumpIfTrueOrPop(ref mut target)) => {
                        *target = end;
                    }
                    _ => panic!("tried to patch invalid instruction"),
                }
            }
        }
    }

    fn end_condition(&mut self, jump_instr: usize) {
        match self.pending_block.pop() {
            Some(PendingBlock::Branch(instr)) => match self.instructions.get_mut(instr) {
                Some(Instruction::JumpIfFalse(ref mut target))
                | Some(Instruction::Jump(ref mut target)) => {
                    *target = jump_instr;
                }
                _ => {}
            },
            _ => panic!("not inside a branch"),
        }
    }

    /// Compiles a statement.
    pub fn compile_stmt(&mut self, stmt: &ast::Stmt<'source>) -> Result<(), Error> {
        match stmt {
            ast::Stmt::Template(t) => {
                self.set_line_from_span(t.span());
                for node in &t.children {
                    self.compile_stmt(node)?;
                }
            }
            ast::Stmt::EmitExpr(expr) => {
                self.compile_emit_expr(expr)?;
            }
            ast::Stmt::EmitRaw(raw) => {
                self.set_line_from_span(raw.span());
                self.add(Instruction::EmitRaw(raw.raw));
            }
            ast::Stmt::ForLoop(for_loop) => {
                self.compile_for_loop(for_loop)?;
            }
            ast::Stmt::IfCond(if_cond) => {
                self.compile_if_stmt(if_cond)?;
            }
            ast::Stmt::WithBlock(with_block) => {
                self.set_line_from_span(with_block.span());
                self.add(Instruction::PushWith);
                for (target, expr) in &with_block.assignments {
                    self.compile_expr(expr)?;
                    self.compile_assignment(target)?;
                }
                for node in &with_block.body {
                    self.compile_stmt(node)?;
                }
                self.add(Instruction::PopFrame);
            }
            ast::Stmt::Set(set) => {
                self.set_line_from_span(set.span());
                self.compile_expr(&set.expr)?;
                self.compile_assignment(&set.target)?;
            }
            ast::Stmt::SetBlock(set_block) => {
                self.set_line_from_span(set_block.span());
                self.add(Instruction::BeginCapture);
                for node in &set_block.body {
                    self.compile_stmt(node)?;
                }
                self.add(Instruction::EndCapture);
                if let Some(ref filter) = set_block.filter {
                    self.compile_expr(filter)?;
                }
                self.compile_assignment(&set_block.target)?;
            }
            ast::Stmt::Block(block) => {
                self.compile_block(block)?;
            }
            ast::Stmt::Extends(extends) => {
                self.set_line_from_span(extends.span());
                self.compile_expr(&extends.name)?;
                self.add(Instruction::LoadBlocks);
            }
            ast::Stmt::Include(include) => {
                self.set_line_from_span(include.span());
                self.compile_expr(&include.name)?;
                self.add_with_span(Instruction::Include(include.ignore_missing), include.span());
            }
            ast::Stmt::AutoEscape(auto_escape) => {
                self.set_line_from_span(auto_escape.span());
                self.compile_expr(&auto_escape.enabled)?;
                self.add(Instruction::PushAutoEscape);
                for node in &auto_escape.body {
                    self.compile_stmt(node)?;
                }
                self.add(Instruction::PopAutoEscape);
            }
            ast::Stmt::FilterBlock(filter_block) => {
                self.set_line_from_span(filter_block.span());
                self.add(Instruction::BeginCapture);
                for node in &filter_block.body {
                    self.compile_stmt(node)?;
                }
                self.add(Instruction::EndCapture);
                self.compile_expr(&filter_block.filter)?;
                self.add(Instruction::Emit);
            }
        }
        Ok(())
    }

    fn compile_block(&mut self, block: &ast::Spanned<ast::Block<'source>>) -> Result<(), Error> {
        self.set_line_from_span(block.span());
        let mut sub = self.new_subgenerator();
        for node in &block.body {
            sub.compile_stmt(node)?;
        }
        let instructions = self.finish_subgenerator(sub);
        self.blocks.insert(block.name, instructions);
        self.add(Instruction::CallBlock(block.name));
        Ok(())
    }

    fn compile_if_stmt(
        &mut self,
        if_cond: &ast::Spanned<ast::IfCond<'source>>,
    ) -> Result<(), Error> {
        self.set_line_from_span(if_cond.span());
        self.compile_expr(&if_cond.expr)?;
        self.start_if();
        for node in &if_cond.true_body {
            self.compile_stmt(node)?;
        }
        if !if_cond.false_body.is_empty() {
            self.start_else();
            for node in &if_cond.false_body {
                self.compile_stmt(node)?;
            }
        }
        self.end_if();
        Ok(())
    }

    fn compile_emit_expr(
        &mut self,
        expr: &ast::Spanned<ast::EmitExpr<'source>>,
    ) -> Result<(), Error> {
        self.set_line_from_span(expr.span());
        if let ast::Expr::Call(call) = &expr.expr {
            if let ast::Expr::Var(var) = &call.expr {
                if var.id == "super" && call.args.is_empty() {
                    self.add_with_span(Instruction::FastSuper, call.span());
                    return Ok(());
                }
                if var.id == "loop" && call.args.len() == 1 {
                    self.compile_expr(&call.args[0])?;
                    self.add(Instruction::FastRecurse);
                    return Ok(());
                }
            }
        }
        self.compile_expr(&expr.expr)?;
        self.add(Instruction::Emit);
        Ok(())
    }

    fn compile_for_loop(
        &mut self,
        for_loop: &ast::Spanned<ast::ForLoop<'source>>,
    ) -> Result<(), Error> {
        self.set_line_from_span(for_loop.span());
        if let Some(ref filter_expr) = for_loop.filter_expr {
            // filter expressions work like a nested for loop without
            // the special loop variable that append into a new list
            // just outside of the loop.
            self.add(Instruction::BuildList(0));
            self.compile_expr(&for_loop.iter)?;
            self.start_for_loop(false, false);
            self.add(Instruction::DupTop);
            self.compile_assignment(&for_loop.target)?;
            self.compile_expr(filter_expr)?;
            self.start_if();
            self.add(Instruction::ListAppend);
            self.start_else();
            self.add(Instruction::DiscardTop);
            self.end_if();
            self.end_for_loop(false);
        } else {
            self.compile_expr(&for_loop.iter)?;
        }
        self.start_for_loop(true, for_loop.recursive);
        self.compile_assignment(&for_loop.target)?;
        for node in &for_loop.body {
            self.compile_stmt(node)?;
        }
        self.end_for_loop(!for_loop.else_body.is_empty());
        if !for_loop.else_body.is_empty() {
            self.start_if();
            for node in &for_loop.else_body {
                self.compile_stmt(node)?;
            }
            self.end_if();
        };
        Ok(())
    }

    /// Compiles an assignment expression.
    pub fn compile_assignment(&mut self, expr: &ast::Expr<'source>) -> Result<(), Error> {
        match expr {
            ast::Expr::Var(var) => {
                self.add(Instruction::StoreLocal(var.id));
            }
            ast::Expr::List(list) => {
                self.push_span(list.span());
                self.add(Instruction::UnpackList(list.items.len()));
                for expr in &list.items {
                    self.compile_assignment(expr)?;
                }
                self.pop_span();
            }
            _ => panic!("bad assignment target"),
        }
        Ok(())
    }

    /// Compiles an expression.
    pub fn compile_expr(&mut self, expr: &ast::Expr<'source>) -> Result<(), Error> {
        match expr {
            ast::Expr::Var(v) => {
                self.set_line_from_span(v.span());
                self.add(Instruction::Lookup(v.id));
            }
            ast::Expr::Const(v) => {
                self.set_line_from_span(v.span());
                self.add(Instruction::LoadConst(v.value.clone()));
            }
            ast::Expr::Slice(s) => {
                self.push_span(s.span());
                self.compile_expr(&s.expr)?;
                if let Some(ref start) = s.start {
                    self.compile_expr(start)?;
                } else {
                    self.add(Instruction::LoadConst(Value::from(0)));
                }
                if let Some(ref stop) = s.stop {
                    self.compile_expr(stop)?;
                } else {
                    self.add(Instruction::LoadConst(Value::from(())));
                }
                if let Some(ref step) = s.step {
                    self.compile_expr(step)?;
                } else {
                    self.add(Instruction::LoadConst(Value::from(1)));
                }
                self.add(Instruction::Slice);
                self.pop_span();
            }
            ast::Expr::UnaryOp(c) => {
                self.set_line_from_span(c.span());
                self.compile_expr(&c.expr)?;
                match c.op {
                    ast::UnaryOpKind::Not => self.add(Instruction::Not),
                    ast::UnaryOpKind::Neg => self.add_with_span(Instruction::Neg, c.span()),
                };
            }
            ast::Expr::BinOp(c) => {
                self.compile_bin_op(c)?;
            }
            ast::Expr::IfExpr(i) => {
                self.set_line_from_span(i.span());
                self.compile_expr(&i.test_expr)?;
                self.start_if();
                self.compile_expr(&i.true_expr)?;
                self.start_else();
                if let Some(ref false_expr) = i.false_expr {
                    self.compile_expr(false_expr)?;
                } else {
                    self.add(Instruction::LoadConst(Value::UNDEFINED));
                }
                self.end_if();
            }
            ast::Expr::Filter(f) => {
                self.push_span(f.span());
                if let Some(ref expr) = f.expr {
                    self.compile_expr(expr)?;
                }
                for arg in &f.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::ApplyFilter(f.name, f.args.len() + 1));
                self.pop_span();
            }
            ast::Expr::Test(f) => {
                self.push_span(f.span());
                self.compile_expr(&f.expr)?;
                for arg in &f.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::PerformTest(f.name, f.args.len() + 1));
                self.pop_span();
            }
            ast::Expr::GetAttr(g) => {
                self.push_span(g.span());
                self.compile_expr(&g.expr)?;
                self.add(Instruction::GetAttr(g.name));
                self.pop_span();
            }
            ast::Expr::GetItem(g) => {
                self.push_span(g.span());
                self.compile_expr(&g.expr)?;
                self.compile_expr(&g.subscript_expr)?;
                self.add(Instruction::GetItem);
                self.pop_span();
            }
            ast::Expr::Call(c) => {
                self.compile_call(c)?;
            }
            ast::Expr::List(l) => {
                if let Some(val) = l.as_const() {
                    self.add(Instruction::LoadConst(val));
                } else {
                    self.set_line_from_span(l.span());
                    for item in &l.items {
                        self.compile_expr(item)?;
                    }
                    self.add(Instruction::BuildList(l.items.len()));
                }
            }
            ast::Expr::Map(m) => {
                if let Some(val) = m.as_const() {
                    self.add(Instruction::LoadConst(val));
                } else {
                    self.set_line_from_span(m.span());
                    assert_eq!(m.keys.len(), m.values.len());
                    for (key, value) in m.keys.iter().zip(m.values.iter()) {
                        self.compile_expr(key)?;
                        self.compile_expr(value)?;
                    }
                    self.add(Instruction::BuildMap(m.keys.len()));
                }
            }
        }
        Ok(())
    }

    fn compile_call(&mut self, c: &ast::Spanned<ast::Call<'source>>) -> Result<(), Error> {
        self.push_span(c.span());
        match c.identify_call() {
            ast::CallType::Function(name) => {
                for arg in &c.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::CallFunction(name, c.args.len()));
            }
            ast::CallType::Block(name) => {
                self.add(Instruction::BeginCapture);
                self.add(Instruction::CallBlock(name));
                self.add(Instruction::EndCapture);
            }
            ast::CallType::Method(expr, name) => {
                self.compile_expr(expr)?;
                for arg in &c.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::CallMethod(name, c.args.len() + 1));
            }
            ast::CallType::Object(expr) => {
                self.compile_expr(expr)?;
                for arg in &c.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::CallObject(c.args.len() + 1));
            }
        };
        self.pop_span();
        Ok(())
    }

    fn compile_bin_op(&mut self, c: &ast::Spanned<ast::BinOp<'source>>) -> Result<(), Error> {
        self.push_span(c.span());
        let instr = match c.op {
            ast::BinOpKind::Eq => Instruction::Eq,
            ast::BinOpKind::Ne => Instruction::Ne,
            ast::BinOpKind::Lt => Instruction::Lt,
            ast::BinOpKind::Lte => Instruction::Lte,
            ast::BinOpKind::Gt => Instruction::Gt,
            ast::BinOpKind::Gte => Instruction::Gte,
            ast::BinOpKind::ScAnd | ast::BinOpKind::ScOr => {
                self.start_sc_bool();
                self.compile_expr(&c.left)?;
                self.sc_bool(matches!(c.op, ast::BinOpKind::ScAnd));
                self.compile_expr(&c.right)?;
                self.end_sc_bool();
                self.pop_span();
                return Ok(());
            }
            ast::BinOpKind::Add => Instruction::Add,
            ast::BinOpKind::Sub => Instruction::Sub,
            ast::BinOpKind::Mul => Instruction::Mul,
            ast::BinOpKind::Div => Instruction::Div,
            ast::BinOpKind::FloorDiv => Instruction::IntDiv,
            ast::BinOpKind::Rem => Instruction::Rem,
            ast::BinOpKind::Pow => Instruction::Pow,
            ast::BinOpKind::Concat => Instruction::StringConcat,
            ast::BinOpKind::In => Instruction::In,
        };
        self.compile_expr(&c.left)?;
        self.compile_expr(&c.right)?;
        self.add(instr);
        self.pop_span();
        Ok(())
    }

    /// Converts the compiler into the instructions.
    pub fn finish(
        self,
    ) -> (
        Instructions<'source>,
        BTreeMap<&'source str, Instructions<'source>>,
    ) {
        assert!(self.pending_block.is_empty());
        (self.instructions, self.blocks)
    }
}
