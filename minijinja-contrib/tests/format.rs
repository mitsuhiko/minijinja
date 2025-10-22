use minijinja::{context, Environment, Value};
use similar_asserts::assert_eq;

fn format_val(env: &Environment, val: impl Into<Value>, spec: &str) -> String {
    let ctx = context!(val=>val.into());
    let fmt = format!("val|fmt('{spec}')");
    let expr = env.compile_expression(&fmt).unwrap();
    expr.eval(ctx).unwrap().to_string()
}

#[test]
fn test_integer_in_decimal() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    // check min-width and zero-padding
    assert_eq!(format_val(&env, 1234, "10"), "      1234");
    assert_eq!(format_val(&env, 123, "010"), "0000000123");
    assert_eq!(format_val(&env, 123, "03"), "123");
    assert_eq!(format_val(&env, 123, "02"), "123");
    // precision is ignored
    assert_eq!(format_val(&env, 123, "4.5"), " 123");
    // align and padding char are ignored for zero-padded number
    assert_eq!(format_val(&env, 123, "=<05"), "00123");

    // check sign
    assert_eq!(format_val(&env, 123, "+"), "+123");
    assert_eq!(format_val(&env, 123, "+05"), "+0123");
    assert_eq!(format_val(&env, -123, "04"), "-123");
    assert_eq!(format_val(&env, -123, "05"), "-0123");
    assert_eq!(format_val(&env, -123, "+05"), "-0123");

    // check align and padding
    assert_eq!(format_val(&env, 123, "=<5"), "123==");
    assert_eq!(format_val(&env, 123, "<5"), "123  ");
    assert_eq!(format_val(&env, 123, "=^5"), "=123=");
    assert_eq!(format_val(&env, 12, "=^5"), "=12==");
    assert_eq!(format_val(&env, 12, "^5"), " 12  ");
    assert_eq!(format_val(&env, 12, "=>5"), "===12");
    assert_eq!(format_val(&env, -12, ">>5"), ">>-12");

    // check min and max
    assert_eq!(format_val(&env, i64::MAX, "+"), "+9223372036854775807");
    assert_eq!(format_val(&env, i64::MIN, "+"), "-9223372036854775808");
    assert_eq!(
        format_val(&env, i64::MIN as i128 - 1, "+"),
        "-9223372036854775809"
    );
    assert_eq!(format_val(&env, u64::MAX, "+"), "+18446744073709551615");
    assert_eq!(
        format_val(&env, u64::MAX as u128 + 1, "+"),
        "+18446744073709551616"
    );
}

#[test]
fn test_integer_in_binary() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    assert_eq!(format_val(&env, 0b1100, "b"), "1100");
    assert_eq!(format_val(&env, 0b11001010, "#b"), "0b11001010");
    assert_eq!(format_val(&env, 0b11001010, "#012b"), "0b0011001010");
    assert_eq!(format_val(&env, 0b11001010, "+#012b"), "+0b011001010");

    assert_eq!(format_val(&env, 16, "#b"), "0b10000");
    assert_eq!(format_val(&env, 16, "+#b"), "+0b10000");
    assert_eq!(format_val(&env, -16, "+#b"), "-0b10000");
    assert_eq!(format_val(&env, -16_i64, "+#b"), "-0b10000");
    assert_eq!(format_val(&env, -16_i128, "#b"), "-0b10000");

    assert_eq!(format_val(&env, 0b1100, "<8b"), "1100    ");
    assert_eq!(format_val(&env, 0b1100, "<#8b"), "0b1100  ");
    assert_eq!(format_val(&env, -12, "^#9b"), " -0b1100 ");
}

#[test]
fn test_integer_in_octal() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    assert_eq!(format_val(&env, 0o71, "o"), "71");
    assert_eq!(format_val(&env, 0o71, "#o"), "0o71");
    assert_eq!(format_val(&env, 0o71, "#06o"), "0o0071");
    assert_eq!(format_val(&env, 0o71, "#6o"), "  0o71");
    assert_eq!(format_val(&env, 0o777, "+#o"), "+0o777");
    assert_eq!(format_val(&env, -0o71, "+#o"), "-0o71");
    assert_eq!(format_val(&env, 7, "0>3o"), "007");
    assert_eq!(format_val(&env, 7, "0>#5o"), "000o7");
}

#[test]
fn test_integer_in_hex() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    assert_eq!(format_val(&env, 0xff, "x"), "ff");
    assert_eq!(format_val(&env, 0xff_u8, "#X"), "0xFF");
    assert_eq!(format_val(&env, 127, "#x"), "0x7f");
    assert_eq!(format_val(&env, 127, "+#x"), "+0x7f");
    assert_eq!(format_val(&env, -127_i8, "#x"), "-0x7f");
    assert_eq!(format_val(&env, -127, "05x"), "-007f");
    assert_eq!(format_val(&env, i32::MAX, "05x"), "7fffffff");
    assert_eq!(format_val(&env, i32::MAX - 1, "=^#12x"), "=0x7ffffffe=");
    assert_eq!(
        format_val(&env, i128::MAX, "#x"),
        "0x7fffffffffffffffffffffffffffffff"
    );
    assert_eq!(format_val(&env, u128::MIN, "#x"), "0x0");
}

#[test]
fn test_integer_as_float() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);
    assert_eq!(format_val(&env, 42, "f"), "42.000000");
    assert_eq!(format_val(&env, 42, "F"), "42.000000");
    assert_eq!(format_val(&env, 42, "e"), "4.200000e1");
    assert_eq!(format_val(&env, 42, "E"), "4.200000E1");
    assert_eq!(format_val(&env, 42, ".3f"), "42.000");
    assert_eq!(format_val(&env, 42, ".2e"), "4.20e1");
    assert_eq!(format_val(&env, 420, ".2e"), "4.20e2");
    assert_eq!(format_val(&env, u64::MAX, ".2e"), "1.84e19");
    assert_eq!(format_val(&env, -42, ".2f"), "-42.00");
}

#[test]
fn test_float() {
    use std::f64::consts::PI;

    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    assert_eq!(format_val(&env, PI, ""), "3.141592653589793");
    assert_eq!(format_val(&env, PI, ".4"), "3.1416");
    assert_eq!(format_val(&env, PI, "f"), "3.141593");
    assert_eq!(format_val(&env, PI, ".2f"), "3.14");
    assert_eq!(format_val(&env, PI, "e"), "3.141593e0");
    assert_eq!(format_val(&env, PI, ".2E"), "3.14E0");
    assert_eq!(format_val(&env, PI, "010.4"), "00003.1416");
    assert_eq!(format_val(&env, PI, "010.4e"), "003.1416e0");
    assert_eq!(format_val(&env, PI, "=^6.2"), "=3.14=");
    assert_eq!(format_val(&env, 1.0 / 5.0, ""), "0.2");
    assert_eq!(format_val(&env, 1.0 / 3.0, ".3"), "0.333");

    assert_eq!(format_val(&env, f64::INFINITY, ""), "inf");
    assert_eq!(format_val(&env, f64::INFINITY, "+"), "+inf");
    assert_eq!(format_val(&env, f64::INFINITY, "f"), "inf");
    assert_eq!(format_val(&env, f64::INFINITY, "e"), "inf");
    assert_eq!(format_val(&env, f64::INFINITY, "06"), "000inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, ""), "-inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "+"), "-inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "f"), "-inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "06"), "-00inf");

    assert_eq!(format_val(&env, 0.0_f64, ""), "0.0");
    assert_eq!(format_val(&env, 0.0_f64, "f"), "0.000000");
    assert_eq!(format_val(&env, 0.0_f64, "+"), "+0.0");
    assert_eq!(format_val(&env, -0.0_f64, "+"), "-0.0");
    assert_eq!(format_val(&env, -0.0_f64, "+05"), "-00.0");

    assert_eq!(format_val(&env, f64::NAN, ""), "NaN");
    assert_eq!(format_val(&env, f64::NAN, "f"), "nan");
    assert_eq!(format_val(&env, f64::NAN, "F"), "NAN");
    assert_eq!(format_val(&env, f64::NAN, "e"), "NaN");
    assert_eq!(format_val(&env, f64::NAN, "E"), "NaN");
}

#[test]
fn test_bool() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    assert_eq!(format_val(&env, false, ""), "false");
    assert_eq!(format_val(&env, true, ""), "true");
    assert_eq!(format_val(&env, true, ".2"), "true");
    assert_eq!(format_val(&env, true, "=^6"), "=true=");
    assert_eq!(format_val(&env, false, "8"), "false   ");

    assert_eq!(format_val(&env, true, "b"), "1");
    assert_eq!(format_val(&env, true, "#x"), "0x1");
    assert_eq!(format_val(&env, false, "#o"), "0o0");
    assert_eq!(format_val(&env, true, "04b"), "0001");
}

#[test]
fn test_str() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    assert_eq!(format_val(&env, "Hello world!", ""), "Hello world!");
    assert_eq!(format_val(&env, "Hello", "<9"), "Hello    ");
    assert_eq!(format_val(&env, "Hello", "^9"), "  Hello  ");
    assert_eq!(format_val(&env, "Hello", ">9"), "    Hello");
    assert_eq!(format_val(&env, "Hello", "=>9"), "====Hello");
    assert_eq!(format_val(&env, "Hello", ".9"), "Hello");
    assert_eq!(format_val(&env, "Hello", "4.2"), "He  ");

    assert_eq!(format_val(&env, "Good", "👍<6"), "Good👍👍");
}

#[test]
fn test_error() {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);

    let expr = env.compile_expression("42|fmt('4.b')").unwrap();
    assert!(expr
        .eval(())
        .unwrap_err()
        .to_string()
        .contains("expecting an integer after '.' in the format spec"));

    let expr = env.compile_expression("42|fmt('#+4')").unwrap();
    assert!(expr
        .eval(())
        .unwrap_err()
        .to_string()
        .contains("invalid character sequence '+4' in the format spec"));
}
