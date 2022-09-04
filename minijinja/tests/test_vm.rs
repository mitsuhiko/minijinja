#![cfg(feature = "unstable_machinery")]
use minijinja::machinery::{simple_eval, Compiler, Instruction, Instructions};
use minijinja::value::Value;

use similar_asserts::assert_eq;

#[test]
fn test_loop() {
    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert(
        "items",
        Value::from((1..=9).into_iter().collect::<Vec<_>>()),
    );

    let mut c = Compiler::new("<unknown>", "");
    c.add(Instruction::Lookup("items"));
    c.start_for_loop(false, false);
    c.add(Instruction::Emit);
    c.end_for_loop(false);
    c.add(Instruction::EmitRaw("!"));

    let mut output = String::new();
    simple_eval(&c.finish().0, ctx, &mut output).unwrap();

    assert_eq!(output, "123456789!");
}

#[test]
fn test_if() {
    for &(val, expectation) in [(true, "true"), (false, "false")].iter() {
        let mut ctx = std::collections::BTreeMap::new();
        ctx.insert("cond", Value::from(val));

        let mut c = Compiler::new("<unknown>", "");
        c.add(Instruction::Lookup("cond"));
        c.start_if();
        c.add(Instruction::EmitRaw("true"));
        c.start_else();
        c.add(Instruction::EmitRaw("false"));
        c.end_if();

        let mut output = String::new();
        simple_eval(&c.finish().0, ctx, &mut output).unwrap();

        assert_eq!(output, expectation);
    }
}

#[test]
fn test_if_branches() {
    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert("false", Value::from(false));
    ctx.insert("nil", Value::from(()));

    let mut c = Compiler::new("<unknown>", "");
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

    let mut output = String::new();
    simple_eval(&c.finish().0, ctx, &mut output).unwrap();

    assert_eq!(output, "yes");
}

#[test]
fn test_basic() {
    let mut user = std::collections::BTreeMap::new();
    user.insert("name", "Peter");
    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert("user", Value::from(user));
    ctx.insert("a", Value::from(42));
    ctx.insert("b", Value::from(23));

    let mut output = String::new();

    let mut i = Instructions::new("", "");
    i.add(Instruction::EmitRaw("Hello "));
    i.add(Instruction::Lookup("user"));
    i.add(Instruction::GetAttr("name"));
    i.add(Instruction::Emit);
    i.add(Instruction::Lookup("a"));
    i.add(Instruction::Lookup("b"));
    i.add(Instruction::Add);
    i.add(Instruction::Neg);
    i.add(Instruction::Emit);

    simple_eval(&i, ctx, &mut output).unwrap();

    assert_eq!(output, "Hello Peter-65");
}

#[test]
fn test_error_info() {
    let mut c = Compiler::new("hello.html", "");
    c.set_line(1);
    c.add(Instruction::EmitRaw("<h1>Hello</h1>\n"));
    c.set_line(2);
    c.add(Instruction::Lookup("a_string"));
    c.add(Instruction::Lookup("an_int"));
    c.add(Instruction::Add);

    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert("a_string", Value::from("foo"));
    ctx.insert("an_int", Value::from(42));

    let err = simple_eval(&c.finish().0, ctx, &mut String::new()).unwrap_err();
    assert_eq!(err.name(), Some("hello.html"));
    assert_eq!(err.line(), Some(2));
}

#[test]
fn test_op_eq() {
    let mut c = Compiler::new("hello.html", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::Eq);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");

    let mut c = Compiler::new("hello.html", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::Eq);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_ne() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from("foo")));
    c.add(Instruction::Ne);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");

    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from("foo")));
    c.add(Instruction::LoadConst(Value::from("foo")));
    c.add(Instruction::Ne);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_lt() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::Lt);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");

    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::Lt);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_gt() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::Gt);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");

    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::Gt);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");
}

#[test]
fn test_op_lte() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::Lte);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");

    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::Lte);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_gte() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(2)));
    c.add(Instruction::Gte);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");

    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::LoadConst(Value::from(1)));
    c.add(Instruction::Gte);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");
}

#[test]
fn test_op_not() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(0)));
    c.add(Instruction::Not);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "true");

    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(true)));
    c.add(Instruction::Not);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_string_concat() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from("foo")));
    c.add(Instruction::LoadConst(Value::from(42)));
    c.add(Instruction::StringConcat);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "foo42");
}

#[test]
fn test_unpacking() {
    let mut c = Compiler::new("<unkown>", "");
    c.add(Instruction::LoadConst(Value::from(vec!["bar", "foo"])));
    c.add(Instruction::UnpackList(2));
    c.add(Instruction::StringConcat);
    c.add(Instruction::Emit);

    let mut output = String::new();
    simple_eval(&c.finish().0, (), &mut output).unwrap();
    assert_eq!(output, "foobar");
}
