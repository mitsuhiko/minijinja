#![cfg(feature = "builtins")]
use minijinja::{context, Environment, Value};
use similar_asserts::assert_eq;

fn format_val(env: &Environment, val: impl Into<Value>, spec: &str) -> String {
    let ctx = context!(val=>val.into());
    let fmt = format!("'%{spec}' | format(val)");
    let expr = env.compile_expression(&fmt).unwrap();
    expr.eval(ctx).unwrap().to_string()
}

fn eval_expr(env: &Environment, expr: &str) -> String {
    let expr = env.compile_expression(expr).unwrap();
    expr.eval(context! {}).unwrap().to_string()
}

fn eval_err_expr(expr: &str) -> String {
    Environment::new()
        .compile_expression(expr)
        .unwrap()
        .eval(())
        .unwrap_err()
        .to_string()
}

#[test]
fn test_format_integer_in_decimal() {
    let env = Environment::new();

    // check min-width and zero-padding
    assert_eq!(eval_expr(&env, "'%10d' | format(1234)"), "      1234");
    assert_eq!(eval_expr(&env, "'%010d' | format(1234)"), "0000001234");
    assert_eq!(
        eval_expr(&env, "'horses: %03d' | format(123)"),
        "horses: 123"
    );
    assert_eq!(
        eval_expr(&env, "'%02d rabbits' | format(123)"),
        "123 rabbits"
    );

    // precision is ignored
    // note: in python, ("%4.5d" % 123) expression results in zero-padded '00123',
    // which seems wrong and so is not repeated in minijinja.
    assert_eq!(eval_expr(&env, "'%4.5d' | format(123)"), " 123");

    // check sign
    assert_eq!(format_val(&env, 123, "+d"), "+123");
    assert_eq!(format_val(&env, 123, "+05d"), "+0123");
    assert_eq!(format_val(&env, -123, "04d"), "-123");
    assert_eq!(format_val(&env, -123, "05d"), "-0123");
    assert_eq!(format_val(&env, -123, "+05d"), "-0123");

    // ' ' (space) flag should add a blank before positive integers...
    assert_eq!(format_val(&env, 123, " d"), " 123");
    assert_eq!(format_val(&env, -123, " d"), "-123");
    // ... unless '+' flag is also used
    assert_eq!(format_val(&env, 123, " +d"), "+123");
    assert_eq!(format_val(&env, -123, " +d"), "-123");

    // check align and padding
    assert_eq!(format_val(&env, 123, "-5d"), "123  ");
    // `-` overrides `0` padding in printf-style format
    assert_eq!(format_val(&env, 123, "-05d"), "123  ");
    // test zero width
    assert_eq!(format_val(&env, 123, "0d"), "123");
    assert_eq!(format_val(&env, 123, "-0d"), "123");

    // check min and max
    assert_eq!(format_val(&env, i64::MAX, "+d"), "+9223372036854775807");
    assert_eq!(format_val(&env, i64::MIN, "+d"), "-9223372036854775808");
    assert_eq!(
        format_val(&env, i64::MIN as i128 - 1, "+d"),
        "-9223372036854775809"
    );
    assert_eq!(format_val(&env, u64::MAX, "+d"), "+18446744073709551615");
    assert_eq!(
        format_val(&env, u64::MAX as u128 + 1, "+d"),
        "+18446744073709551616"
    );

    // test 'i' flag
    assert_eq!(eval_expr(&env, "'horses: %i' | format(123)"), "horses: 123");
    // len modifiers must be ignored
    assert_eq!(
        eval_expr(&env, "'%ld, %0hi, %Ld' | format(1,2,3)"),
        "1, 2, 3"
    );
}

#[test]
fn test_format_integer_as_string() {
    let env = Environment::new();

    assert_eq!(format_val(&env, 123, "s"), "123");
    assert_eq!(format_val(&env, -123, "s"), "-123");
    // Python ignores '+' for positive numbers
    assert_eq!(format_val(&env, 123, "+s"), "123");
    assert_eq!(format_val(&env, -123, "+s"), "-123");
}

#[test]
fn test_format_integer_in_octal() {
    let env = Environment::new();

    assert_eq!(format_val(&env, 0o71, "o"), "71");
    assert_eq!(format_val(&env, 0o71, "#o"), "0o71");
    assert_eq!(format_val(&env, 0o71, "#06o"), "0o0071");
    assert_eq!(format_val(&env, 0o71, "#6o"), "  0o71");
    assert_eq!(format_val(&env, 0o777, "+#o"), "+0o777");
    assert_eq!(format_val(&env, -0o71, "+#o"), "-0o71");
    assert_eq!(format_val(&env, 7, "-3o"), "7  ");
    assert_eq!(format_val(&env, 7, "-#5o"), "0o7  ");
}

#[test]
fn test_format_integer_in_hex() {
    let env = Environment::new();

    assert_eq!(format_val(&env, 0xff, "x"), "ff");
    assert_eq!(format_val(&env, 0xff_u8, "#X"), "0XFF");
    assert_eq!(format_val(&env, 127, "#x"), "0x7f");
    assert_eq!(format_val(&env, 127, "+#x"), "+0x7f");
    assert_eq!(format_val(&env, -127_i8, "#x"), "-0x7f");
    assert_eq!(format_val(&env, -127, "05x"), "-007f");
    assert_eq!(format_val(&env, i32::MAX, "05x"), "7fffffff");
    assert_eq!(format_val(&env, i32::MAX - 1, "-#12x"), "0x7ffffffe  ");
    assert_eq!(
        format_val(&env, i128::MAX, "#x"),
        "0x7fffffffffffffffffffffffffffffff"
    );
    assert_eq!(format_val(&env, u128::MIN, "#x"), "0x0");
}

#[test]
fn test_format_integer_as_float() {
    let env = Environment::new();
    assert_eq!(format_val(&env, 42, "f"), "42.000000");
    assert_eq!(format_val(&env, 42, "F"), "42.000000");
    assert_eq!(format_val(&env, 42, "e"), "4.200000e+01");
    assert_eq!(format_val(&env, 42, "E"), "4.200000E+01");
    assert_eq!(format_val(&env, 42, ".3f"), "42.000");
    assert_eq!(format_val(&env, 42, ".2e"), "4.20e+01");
    assert_eq!(format_val(&env, 420, ".2e"), "4.20e+02");
    assert_eq!(format_val(&env, u64::MAX, ".2e"), "1.84e+19");
    assert_eq!(format_val(&env, -42, ".2f"), "-42.00");

    // test zero precision
    assert_eq!(format_val(&env, 42, ".0f"), "42");
    assert_eq!(format_val(&env, 42, "#.0f"), "42.");
    assert_eq!(format_val(&env, 42, ".0e"), "4e+01");
    assert_eq!(format_val(&env, 42, "#.0e"), "4.e+01");
    assert_eq!(format_val(&env, 42, ".0E"), "4E+01");
    assert_eq!(format_val(&env, 42, "#.0E"), "4.E+01");
}

#[test]
fn test_format_float() {
    use std::f64::consts::PI;

    let env = Environment::new();

    // test 'f' and 'e' conversions with different width and zero padding
    assert_eq!(format_val(&env, PI, "f"), "3.141593");
    assert_eq!(format_val(&env, PI, ".2f"), "3.14");
    assert_eq!(format_val(&env, PI, "e"), "3.141593e+00");
    assert_eq!(format_val(&env, PI, ".2E"), "3.14E+00");
    assert_eq!(format_val(&env, PI, "010.4f"), "00003.1416");
    assert_eq!(format_val(&env, PI, "012.4e"), "003.1416e+00");
    assert_eq!(format_val(&env, PI, "6.2f"), "  3.14");
    assert_eq!(format_val(&env, PI, "-6.2f"), "3.14  ");
    assert_eq!(format_val(&env, 1.0 / 5.0, "f"), "0.200000");
    assert_eq!(format_val(&env, 1.0 / 3.0, ".3f"), "0.333");

    // test zero precision
    assert_eq!(format_val(&env, PI, ".0f"), "3");
    assert_eq!(format_val(&env, PI, "#.0f"), "3.");
    assert_eq!(format_val(&env, PI, ".0e"), "3e+00");
    assert_eq!(format_val(&env, PI, "#.0e"), "3.e+00");

    // test inf and -inf
    assert_eq!(format_val(&env, f64::INFINITY, "f"), "inf");
    assert_eq!(format_val(&env, f64::INFINITY, "+f"), "+inf");
    assert_eq!(format_val(&env, f64::INFINITY, "e"), "inf");
    assert_eq!(format_val(&env, f64::INFINITY, "06e"), "000inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "+f"), "-inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "f"), "-inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "06f"), "-00inf");

    // test +0.0 and -0.0
    assert_eq!(format_val(&env, 0.0_f64, "f"), "0.000000");
    assert_eq!(format_val(&env, 0.0_f64, "+f"), "+0.000000");
    assert_eq!(format_val(&env, -0.0_f64, "+f"), "-0.000000");
    assert_eq!(format_val(&env, -0.0_f64, "+05.1f"), "-00.0");

    // test nan
    assert_eq!(format_val(&env, f64::NAN, "f"), "nan");
    assert_eq!(format_val(&env, f64::NAN, "F"), "NAN");
    assert_eq!(format_val(&env, f64::NAN, "e"), "nan");
    assert_eq!(format_val(&env, f64::NAN, "E"), "NAN");

    assert_eq!(format_val(&env, f64::MIN, ".4e"), "-1.7977e+308");
    assert_eq!(format_val(&env, f64::MIN_POSITIVE, ".4e"), "2.2251e-308");
    assert_eq!(format_val(&env, f64::MAX, ".2e"), "1.80e+308");
    assert_eq!(format_val(&env, f32::MIN, ".5E"), "-3.40282E+38");
    assert_eq!(format_val(&env, f32::MIN_POSITIVE, ".4E"), "1.1755E-38");
    assert_eq!(format_val(&env, f32::MAX, ".2E"), "3.40E+38");
}

#[test]
fn test_format_general() {
    let env = Environment::new();
    assert_eq!(format_val(&env, 123.456, "g"), "123.456");
    assert_eq!(format_val(&env, 123.000, "g"), "123");
    assert_eq!(format_val(&env, 120.0, "g"), "120");
    assert_eq!(format_val(&env, 1234.012, "g"), "1234.01");
    assert_eq!(format_val(&env, 12345.1234, "g"), "12345.1");
    assert_eq!(format_val(&env, 123456.1234, "g"), "123456");
    assert_eq!(format_val(&env, 1234564.1234, "g"), "1.23456e+06");
    assert_eq!(format_val(&env, 12345678.1234, "g"), "1.23457e+07");
    assert_eq!(format_val(&env, 12345000.00, "g"), "1.2345e+07");
    assert_eq!(format_val(&env, 12345000.00, "G"), "1.2345E+07");

    assert_eq!(format_val(&env, 0.123456, "g"), "0.123456");
    assert_eq!(format_val(&env, 0.0123456, "g"), "0.0123456");
    assert_eq!(format_val(&env, 0.00123456, "g"), "0.00123456");
    assert_eq!(format_val(&env, 0.000123456, "g"), "0.000123456");
    assert_eq!(format_val(&env, 0.000123, "g"), "0.000123");
    assert_eq!(format_val(&env, 0.0000123456, "g"), "1.23456e-05");
    assert_eq!(format_val(&env, 0.0000123, "g"), "1.23e-05");
    assert_eq!(format_val(&env, 0.0000123456789, "g"), "1.23457e-05");
    assert_eq!(format_val(&env, 0.0000000456789, "g"), "4.56789e-08");
    assert_eq!(format_val(&env, 0.0000000456780, "g"), "4.5678e-08");
    assert_eq!(format_val(&env, 0.00000004, "g"), "4e-08");
    assert_eq!(format_val(&env, 0.00000004, "#g"), "4.00000e-08");

    assert_eq!(format_val(&env, 123.456, ".2g"), "1.2e+02");
    assert_eq!(format_val(&env, 123.456, ".2g"), "1.2e+02");
    assert_eq!(format_val(&env, 1000, ".4g"), "1000");
    assert_eq!(format_val(&env, 1000, ".3g"), "1e+03");
    assert_eq!(format_val(&env, 1000.00, ".4g"), "1000");
    assert_eq!(format_val(&env, 1000.10, ".5g"), "1000.1");
    assert_eq!(format_val(&env, 1000.10, ".6g"), "1000.1");
    assert_eq!(format_val(&env, 123456789.1234, ".6g"), "1.23457e+08");
    assert_eq!(format_val(&env, 123456789.1234, ".10g"), "123456789.1");
    assert_eq!(format_val(&env, 0.123456, ".2g"), "0.12");
    assert_eq!(format_val(&env, 0.00012345, ".4g"), "0.0001234");
    assert_eq!(format_val(&env, 0.000012345, ".4g"), "1.234e-05");
    assert_eq!(format_val(&env, 123.456, ".0g"), "1e+02");
    assert_eq!(format_val(&env, 0.456, ".0g"), "0.5");

    assert_eq!(format_val(&env, -123.456, "g"), "-123.456");
    assert_eq!(format_val(&env, -123.456, ".2g"), "-1.2e+02");
    assert_eq!(format_val(&env, -1000, ".4g"), "-1000");
    assert_eq!(format_val(&env, -100.100, ".4g"), "-100.1");
    assert_eq!(format_val(&env, -123456789.1234, "15g"), "   -1.23457e+08");
    assert_eq!(format_val(&env, -123456789.1234, "015g"), "-0001.23457e+08");
    assert_eq!(format_val(&env, -0.123456, "g"), "-0.123456");
    assert_eq!(format_val(&env, -0.0000123456, "g"), "-1.23456e-05");
    assert_eq!(format_val(&env, -0.0000123456, "09.2g"), "-01.2e-05");

    assert_eq!(format_val(&env, 0.0_f64, "g"), "0");
    assert_eq!(format_val(&env, -0.0_f64, "g"), "-0");
    assert_eq!(format_val(&env, f64::NAN, "g"), "nan");
    assert_eq!(format_val(&env, f64::NAN, "G"), "NAN");
    assert_eq!(format_val(&env, f64::INFINITY, "g"), "inf");
    assert_eq!(format_val(&env, f64::INFINITY, "G"), "INF");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "g"), "-inf");
    assert_eq!(format_val(&env, f64::NEG_INFINITY, "G"), "-INF");
}

#[test]
fn test_format_bool() {
    let env = Environment::new();

    assert_eq!(format_val(&env, false, "s"), "false");
    assert_eq!(format_val(&env, true, "s"), "true");
    assert_eq!(format_val(&env, true, ".2d"), "1");
    assert_eq!(format_val(&env, true, "-5d"), "1    ");
    assert_eq!(format_val(&env, false, "5d"), "    0");

    assert_eq!(format_val(&env, true, "d"), "1");
    assert_eq!(format_val(&env, true, "#x"), "0x1");
    assert_eq!(format_val(&env, false, "#o"), "0o0");
    assert_eq!(format_val(&env, true, "04d"), "0001");
}

#[test]
fn test_format_str() {
    let env = Environment::new();

    assert_eq!(format_val(&env, "Hello world!", "s"), "Hello world!");
    assert_eq!(format_val(&env, "Hello", "9s"), "    Hello");
    assert_eq!(format_val(&env, "Hello", "-9s"), "Hello    ");
    assert_eq!(format_val(&env, "Hello", ".2s"), "He");
    assert_eq!(format_val(&env, "Hello", "4.2s"), "  He");

    assert_eq!(
        eval_expr(&env, "'Hello %s and %s!'|format('Bob','Alice')"),
        "Hello Bob and Alice!"
    );
}

#[test]
fn test_format_with_mapping_arg() {
    let env = Environment::new();

    let mapping = "{ 'key' : 42, '1' : 'magic' }";
    assert_eq!(
        eval_expr(
            &env,
            &format!("'read from mapping: %(key)d' | format({mapping})")
        ),
        "read from mapping: 42"
    );
    assert_eq!(
        eval_expr(
            &env,
            &format!("'%(key)d is %(1)s number' | format({mapping})")
        ),
        "42 is magic number"
    );

    let expr = env
        .compile_expression("'%(key)d must be in mapping' | format(42)")
        .unwrap();
    assert!(expr
        .eval(())
        .unwrap_err()
        .to_string()
        .contains("format argument must be a mapping"));

    let with_bad_key = format!("'%(bad-key)d must be in mapping' | format({mapping})");
    let expr = env.compile_expression(&with_bad_key).unwrap();
    assert!(expr
        .eval(())
        .unwrap_err()
        .to_string()
        .contains("missing an argument for format spec"));
}

#[test]
fn test_format_escaping() {
    let env = Environment::new();

    assert_eq!(eval_expr(&env, "'%% %d'|format(42)"), "% 42");
    assert_eq!(eval_expr(&env, "'%%%d'|format(42)"), "%42");
    assert_eq!(
        eval_expr(&env, "'first %% second %% consecutive %%%% %d'|format(42)"),
        "first % second % consecutive %% 42"
    );
}

#[test]
fn test_format_error() {
    assert!(
        eval_err_expr("'missing type: %04' | format('arg')").contains("missing conversion type")
    );

    assert!(eval_err_expr("'missing type: %a' | format('arg')")
        .contains("invalid conversion type 'a' in format spec"));

    assert!(eval_err_expr("'% %s' | format('arg')")
        .contains("invalid conversion type '%' in format spec"));
}
