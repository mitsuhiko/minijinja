use std::collections::BTreeMap;

use crate::ast;
use crate::error::Error;
use crate::instructions::{Instruction, Instructions};
use crate::tokens::Span;
use crate::value::Value;

/// Represents an open block of code that does not yet have updated
/// jump targets.
#[derive(Debug)]
enum PendingBlock {
    Branch(usize),
    Loop(usize),
    ScBool(Vec<usize>),
}

/// Provides a convenient interface to creating instructions for the VM.
#[derive(Debug)]
pub struct Compiler<'source> {
    instructions: Instructions<'source>,
    blocks: BTreeMap<&'source str, Instructions<'source>>,
    pending_block: Vec<PendingBlock>,
    current_file: &'source str,
    current_line: usize,
}

impl<'source> Default for Compiler<'source> {
    fn default() -> Self {
        Compiler::new()
    }
}

impl<'source> Compiler<'source> {
    /// Creates an empty compiler
    pub fn new() -> Compiler<'source> {
        Compiler {
            instructions: Instructions::default(),
            blocks: BTreeMap::new(),
            pending_block: Vec::new(),
            current_file: "<unknown>",
            current_line: 0,
        }
    }

    /// Sets the current location's filename.
    pub fn set_file(&mut self, filename: &'source str) {
        self.current_file = filename;
    }

    /// Sets the current location's line.
    pub fn set_line(&mut self, lineno: usize) {
        self.current_line = lineno;
    }

    /// Sets location from span.
    pub fn set_location_from_span(&mut self, span: Span) {
        self.set_line(span.start_line);
    }

    /// Add a simple instruction.
    pub fn add(&mut self, instr: Instruction<'source>) -> usize {
        self.instructions
            .add_with_location(instr, self.current_file, self.current_line)
    }

    /// Returns the next instruction index.
    pub fn next_instruction(&self) -> usize {
        self.instructions.len()
    }

    /// Starts a for loop
    pub fn start_for_loop(&mut self, target_name: &'source str) {
        self.add(Instruction::PushLoop(target_name));
        let iter_instr = self.add(Instruction::Iterate(!0));
        self.pending_block.push(PendingBlock::Loop(iter_instr));
    }

    /// Ends the open for loop
    pub fn end_for_loop(&mut self) {
        match self.pending_block.pop() {
            Some(PendingBlock::Loop(iter_instr)) => {
                self.add(Instruction::Jump(iter_instr));
                let loop_end = self.add(Instruction::PopFrame);
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
                self.set_location_from_span(t.span());
                for node in &t.children {
                    self.compile_stmt(node)?;
                }
            }
            ast::Stmt::EmitExpr(expr) => {
                self.set_location_from_span(expr.span());
                self.compile_expr(&expr.expr)?;
                self.add(Instruction::Emit);
            }
            ast::Stmt::EmitRaw(raw) => {
                self.set_location_from_span(raw.span());
                self.add(Instruction::EmitRaw(raw.raw));
            }
            ast::Stmt::ForLoop(for_loop) => {
                self.set_location_from_span(for_loop.span());
                self.compile_expr(&for_loop.iter)?;
                self.start_for_loop(for_loop.target);
                for node in &for_loop.body {
                    self.compile_stmt(node)?;
                }
                self.end_for_loop();
            }
            ast::Stmt::IfCond(if_cond) => {
                self.set_location_from_span(if_cond.span());
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
            }
            ast::Stmt::WithBlock(with_block) => {
                self.set_location_from_span(with_block.span());
                for (target, expr) in &with_block.assignments {
                    self.add(Instruction::LoadConst(Value::from(*target)));
                    self.compile_expr(expr)?;
                }
                self.add(Instruction::BuildMap(with_block.assignments.len()));
                self.add(Instruction::PushContext);
                for node in &with_block.body {
                    self.compile_stmt(node)?;
                }
                self.add(Instruction::PopFrame);
            }
            ast::Stmt::Block(block) => {
                self.set_location_from_span(block.span());
                let mut sub_compiler = Compiler::new();
                sub_compiler.set_file(self.current_file);
                sub_compiler.set_line(self.current_line);
                for node in &block.body {
                    sub_compiler.compile_stmt(node)?;
                }

                let (instructions, blocks) = sub_compiler.finish();
                self.blocks.extend(blocks.into_iter());
                self.blocks.insert(block.name, instructions);
                self.add(Instruction::CallBlock(block.name));
            }
            ast::Stmt::Extends(extends) => {
                self.set_location_from_span(extends.span());
                self.compile_expr(&extends.name)?;
                self.add(Instruction::LoadBlocks);
            }
            ast::Stmt::AutoEscape(auto_escape) => {
                self.set_location_from_span(auto_escape.span());
                self.compile_expr(&auto_escape.enabled)?;
                self.add(Instruction::PushAutoEscape);
                for node in &auto_escape.body {
                    self.compile_stmt(node)?;
                }
                self.add(Instruction::PopAutoEscape);
            }
        }
        Ok(())
    }

    /// Compiles an expression.
    pub fn compile_expr(&mut self, expr: &ast::Expr<'source>) -> Result<(), Error> {
        match expr {
            ast::Expr::Var(v) => {
                self.set_location_from_span(v.span());
                self.add(Instruction::Lookup(v.id));
            }
            ast::Expr::Const(v) => {
                self.set_location_from_span(v.span());
                self.add(Instruction::LoadConst(v.value.clone()));
            }
            ast::Expr::UnaryOp(c) => {
                self.set_location_from_span(c.span());
                self.compile_expr(&c.expr)?;
                self.add(match c.op {
                    ast::UnaryOpKind::Not => Instruction::Not,
                    ast::UnaryOpKind::Neg => Instruction::Neg,
                });
            }
            ast::Expr::BinOp(c) => {
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
                        return Ok(());
                    }
                    ast::BinOpKind::Add => Instruction::Add,
                    ast::BinOpKind::Sub => Instruction::Sub,
                    ast::BinOpKind::Mul => Instruction::Mul,
                    ast::BinOpKind::Div => Instruction::Div,
                    // TODO: floor div maps to normal div for now
                    ast::BinOpKind::FloorDiv => Instruction::Div,
                    ast::BinOpKind::Rem => Instruction::Rem,
                    ast::BinOpKind::Pow => Instruction::Pow,
                    ast::BinOpKind::Concat => Instruction::StringConcat,
                };
                self.set_location_from_span(c.span());
                self.compile_expr(&c.left)?;
                self.compile_expr(&c.right)?;
                self.add(instr);
            }
            ast::Expr::Filter(f) => {
                self.set_location_from_span(f.span());
                self.compile_expr(&f.expr)?;
                for arg in &f.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::BuildList(f.args.len()));
                self.add(Instruction::ApplyFilter(f.name));
            }
            ast::Expr::Test(f) => {
                self.set_location_from_span(f.span());
                self.compile_expr(&f.expr)?;
                for arg in &f.args {
                    self.compile_expr(arg)?;
                }
                self.add(Instruction::BuildList(f.args.len()));
                self.add(Instruction::PerformTest(f.name));
            }
            ast::Expr::GetAttr(g) => {
                self.set_location_from_span(g.span());
                self.compile_expr(&g.expr)?;
                self.add(Instruction::GetAttr(g.name));
            }
            ast::Expr::GetItem(g) => {
                self.set_location_from_span(g.span());
                self.compile_expr(&g.expr)?;
                self.compile_expr(&g.subscript_expr)?;
                self.add(Instruction::GetItem);
            }
            ast::Expr::Call(c) => {
                self.set_location_from_span(c.span());
                match c.identify_call() {
                    ast::CallType::Function(name) => {
                        for arg in &c.args {
                            self.compile_expr(arg)?;
                        }
                        self.add(Instruction::BuildList(c.args.len()));
                        self.add(Instruction::CallFunction(name));
                    }
                    ast::CallType::Method(expr, name) => {
                        self.compile_expr(expr)?;
                        for arg in &c.args {
                            self.compile_expr(arg)?;
                        }
                        self.add(Instruction::BuildList(c.args.len()));
                        self.add(Instruction::CallMethod(name));
                    }
                    ast::CallType::Object(expr) => {
                        self.compile_expr(expr)?;
                        self.add(Instruction::CallObject);
                    }
                }
            }
            ast::Expr::List(l) => {
                self.set_location_from_span(l.span());
                for item in &l.items {
                    self.compile_expr(item)?;
                }
                self.add(Instruction::BuildList(l.items.len()));
            }
            ast::Expr::Map(m) => {
                self.set_location_from_span(m.span());
                assert_eq!(m.keys.len(), m.values.len());
                for (key, value) in m.keys.iter().zip(m.values.iter()) {
                    self.compile_expr(key)?;
                    self.compile_expr(value)?;
                }
                self.add(Instruction::BuildMap(m.keys.len()));
            }
        }
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
