#![cfg(feature = "unstable_machinery")]
use std::collections::BTreeMap;

use minijinja::machinery::{make_string_output, CodeGenerator, Instruction, Instructions, Vm};
use minijinja::value::Value;
use minijinja::{AutoEscape, Environment, Error};

use similar_asserts::assert_eq;

pub fn simple_eval<S: serde::Serialize>(
    instructions: &Instructions<'_>,
    ctx: S,
) -> Result<String, Error> {
    let env = Environment::new();
    let empty_blocks = BTreeMap::new();
    let vm = Vm::new(&env);
    let root = Value::from_serialize(&ctx);
    let mut rv = String::new();
    let mut output = make_string_output(&mut rv);
    vm.eval(
        instructions,
        root,
        &empty_blocks,
        &mut output,
        AutoEscape::None,
    )?;
    Ok(rv)
}

#[test]
fn test_loop() {
    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert("items", Value::from((1..=9).collect::<Vec<_>>()));

    let mut c = CodeGenerator::new("<unknown>", "");
    let items_id = c.register_str("items");
    c.add(Instruction::Lookup(items_id));
    c.start_for_loop(false, false);
    c.add(Instruction::Emit);
    c.end_for_loop(false);
    let bang_id = c.register_str("!");
    c.add(Instruction::EmitRaw(bang_id));

    let output = simple_eval(&c.finish().0, ctx).unwrap();

    assert_eq!(output, "123456789!");
}

#[test]
fn test_if() {
    for &(val, expectation) in [(true, "true"), (false, "false")].iter() {
        let mut ctx = std::collections::BTreeMap::new();
        ctx.insert("cond", Value::from(val));

        let mut c = CodeGenerator::new("<unknown>", "");
        let cond_id = c.register_str("cond");
        c.add(Instruction::Lookup(cond_id));
        c.start_if();
        let true_id = c.register_str("true");
        c.add(Instruction::EmitRaw(true_id));
        c.start_else();
        let false_id = c.register_str("false");
        c.add(Instruction::EmitRaw(false_id));
        c.end_if();

        let output = simple_eval(&c.finish().0, ctx).unwrap();

        assert_eq!(output, expectation);
    }
}

#[test]
fn test_if_branches() {
    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert("false", Value::from(false));
    ctx.insert("nil", Value::from(()));

    let mut c = CodeGenerator::new("<unknown>", "");
    let false_id = c.register_str("false");
    c.add(Instruction::Lookup(false_id));
    c.start_if();
    let nope1_id = c.register_str("nope1");
    c.add(Instruction::EmitRaw(nope1_id));
    c.start_else();
    let nil_id = c.register_str("nil");
    c.add(Instruction::Lookup(nil_id));
    c.start_if();
    c.add(Instruction::EmitRaw(nope1_id));
    c.start_else();
    let yes_id = c.register_str("yes");
    c.add(Instruction::EmitRaw(yes_id));
    c.end_if();
    c.end_if();

    let output = simple_eval(&c.finish().0, ctx).unwrap();

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

    let mut c = CodeGenerator::new("hello.html", "");
    let hello_id = c.register_str("Hello ");
    c.add(Instruction::EmitRaw(hello_id));
    let user_id = c.register_str("user");
    c.add(Instruction::Lookup(user_id));
    let name_id = c.register_str("name");
    c.add(Instruction::GetAttr(name_id));
    c.add(Instruction::Emit);
    let a_id = c.register_str("a");
    c.add(Instruction::Lookup(a_id));
    let b_id = c.register_str("b");
    c.add(Instruction::Lookup(b_id));
    c.add(Instruction::Add);
    c.add(Instruction::Neg);
    c.add(Instruction::Emit);
    let i = c.finish().0;

    let output = simple_eval(&i, ctx).unwrap();
    assert_eq!(output, "Hello Peter-65");
}

#[test]
fn test_error_info() {
    let mut c = CodeGenerator::new("hello.html", "");
    c.set_line(1);
    let hello_id = c.register_str("<h1>Hello</h1>\n");
    c.add(Instruction::EmitRaw(hello_id));
    c.set_line(2);
    let a_string_id = c.register_str("a_string");
    c.add(Instruction::Lookup(a_string_id));
    let an_int_id = c.register_str("an_int");
    c.add(Instruction::Lookup(an_int_id));
    c.add(Instruction::Add);

    let mut ctx = std::collections::BTreeMap::new();
    ctx.insert("a_string", Value::from("foo"));
    ctx.insert("an_int", Value::from(42));

    let err = simple_eval(&c.finish().0, ctx).unwrap_err();
    assert_eq!(err.name(), Some("hello.html"));
    assert_eq!(err.line(), Some(2));
}

#[test]
fn test_op_eq() {
    let mut c = CodeGenerator::new("hello.html", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(1));
    c.add(Instruction::Eq);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");

    let mut c = CodeGenerator::new("hello.html", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(2));
    c.add(Instruction::Eq);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_ne() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from("foo"));
    c.add(Instruction::Ne);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");

    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from("foo"));
    c.add_const(Value::from("foo"));
    c.add(Instruction::Ne);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_lt() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(2));
    c.add(Instruction::Lt);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");

    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(2));
    c.add_const(Value::from(1));
    c.add(Instruction::Lt);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_gt() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(2));
    c.add(Instruction::Gt);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");

    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(2));
    c.add_const(Value::from(1));
    c.add(Instruction::Gt);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");
}

#[test]
fn test_op_lte() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(1));
    c.add(Instruction::Lte);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");

    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(2));
    c.add_const(Value::from(1));
    c.add(Instruction::Lte);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_op_gte() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(2));
    c.add(Instruction::Gte);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");

    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(1));
    c.add_const(Value::from(1));
    c.add(Instruction::Gte);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");
}

#[test]
fn test_op_not() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(0));
    c.add(Instruction::Not);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "true");

    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(true));
    c.add(Instruction::Not);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "false");
}

#[test]
fn test_string_concat() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from("foo"));
    c.add_const(Value::from(42));
    c.add(Instruction::StringConcat);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "foo42");
}

#[test]
fn test_unpacking() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from(vec!["bar", "foo"]));
    c.add(Instruction::UnpackList(2));
    c.add(Instruction::StringConcat);
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "foobar");
}

#[test]
fn test_call_object() {
    let mut c = CodeGenerator::new("<unknown>", "");
    c.add_const(Value::from_function(|a: u64| 42 + a));
    c.add_const(Value::from(23i32));
    c.add(Instruction::CallObject(Some(2)));
    c.add(Instruction::Emit);

    let output = simple_eval(&c.finish().0, ()).unwrap();
    assert_eq!(output, "65");
}

#[test]
#[cfg(feature = "stacker")]
fn test_deep_recursion() {
    use minijinja::context;

    let mut env = Environment::new();
    let limit = if cfg!(target_arch = "wasm32") {
        500
    } else {
        10000
    };
    env.set_recursion_limit(limit * 7);
    let tmpl = env
        .template_from_str(
            r#"
        {%- macro foo(i) -%}
            {%- if i < limit %}{{ i }}|{{ foo(i + 1) }}{%- endif -%}
        {%- endmacro -%}
        {{- foo(0) -}}
    "#,
        )
        .unwrap();
    let rv = tmpl.render(context!(limit)).unwrap();
    let pieces = rv
        .split('|')
        .filter_map(|x| x.parse::<usize>().ok())
        .collect::<Vec<_>>();
    assert_eq!(pieces, (0..limit).collect::<Vec<_>>());
}
