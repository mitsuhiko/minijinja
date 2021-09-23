#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::{Compiler, Instruction};
use minijinja::value::Value;

#[test]
fn test_for_loop() {
    let mut c = Compiler::new();
    c.add(Instruction::Lookup("items"));
    c.start_for_loop("x");
    c.add(Instruction::Lookup("x"));
    c.add(Instruction::Emit);
    c.end_for_loop();
    c.add(Instruction::EmitRaw("!"));

    insta::assert_debug_snapshot!(&c);
}

#[test]
fn test_if_branches() {
    let mut c = Compiler::new();
    c.add(Instruction::Lookup("false"));
    c.start_if();
    c.add(Instruction::EmitRaw("nope1"));
    c.start_else();
    c.add(Instruction::Lookup("nil"));
    c.start_if();
    c.add(Instruction::EmitRaw("nope1"));
    c.start_else();
    c.add(Instruction::EmitRaw("yes"));
    c.end_if();
    c.end_if();

    insta::assert_debug_snapshot!(&c);
}

#[test]
fn test_bool_ops() {
    let mut c = Compiler::new();

    c.start_sc_bool();
    c.add(Instruction::Lookup("first"));
    c.sc_bool(true);
    c.add(Instruction::Lookup("second"));
    c.sc_bool(false);
    c.add(Instruction::Lookup("third"));
    c.end_sc_bool();

    insta::assert_debug_snapshot!(&c);
}

#[test]
fn test_const() {
    let mut c = Compiler::new();

    c.add(Instruction::LoadConst(Value::from("a")));
    c.add(Instruction::LoadConst(Value::from(42)));
    c.add(Instruction::StringConcat);

    insta::assert_debug_snapshot!(&c);
}
