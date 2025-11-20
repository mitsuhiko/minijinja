#![cfg(feature = "pycompat")]
use minijinja::{Environment, Value};
use minijinja_contrib::pycompat::unknown_method_callback;
use similar_asserts::assert_eq;

fn eval_expr(expr: &str) -> Value {
    let mut env = Environment::new();
    env.set_unknown_method_callback(unknown_method_callback);
    env.compile_expression(expr).unwrap().eval(()).unwrap()
}

fn eval_err_expr(expr: &str) -> String {
    let mut env = Environment::new();
    env.set_unknown_method_callback(unknown_method_callback);
    env.compile_expression(expr)
        .unwrap()
        .eval(())
        .unwrap_err()
        .to_string()
}

#[test]
fn test_format_sign() {
    assert_eq!(eval_expr("'{:+d}'.format(123)").as_str(), Some("+123"));
    assert_eq!(eval_expr("'{:+d}'.format(-123)").as_str(), Some("-123"));
    assert_eq!(eval_expr("'{}'.format(-123)").as_str(), Some("-123"));
    assert_eq!(eval_expr("'{: d}'.format(123)").as_str(), Some(" 123"));
    assert_eq!(eval_expr("'{: d}'.format(-123)").as_str(), Some("-123"));
}

#[test]
fn test_format_align_padding() {
    assert_eq!(eval_expr("'{:=<5}'.format(123)").as_str(), Some("123=="));
    assert_eq!(eval_expr("'{:<5}'.format(123)").as_str(), Some("123  "));
    assert_eq!(eval_expr("'{:=^5}'.format(123)").as_str(), Some("=123="));
    assert_eq!(eval_expr("'{:=^5}'.format(12)").as_str(), Some("=12=="));
    assert_eq!(eval_expr("'{:^5}'.format(12)").as_str(), Some(" 12  "));
    assert_eq!(eval_expr("'{:=>5}'.format(12)").as_str(), Some("===12"));
    assert_eq!(eval_expr("'{:>>5d}'.format(-12)").as_str(), Some(">>-12"));
    assert_eq!(
        eval_expr("'{:👍<6}'.format('Good')").as_str(),
        Some("Good👍👍")
    );
}

#[test]
fn test_format_binary_octal_hex() {
    // check different radix
    assert_eq!(eval_expr("'{:b}'.format(16)").as_str(), Some("10000"));
    assert_eq!(eval_expr("'{:#b}'.format(17)").as_str(), Some("0b10001"));
    assert_eq!(eval_expr("'{:#8b}'.format(17)").as_str(), Some(" 0b10001"));
    assert_eq!(eval_expr("'{:<#8b}'.format(17)").as_str(), Some("0b10001 "));
    assert_eq!(eval_expr("'{:#08b}'.format(17)").as_str(), Some("0b010001"));
    assert_eq!(
        eval_expr("'{:+#08b}'.format(17)").as_str(),
        Some("+0b10001")
    );
    assert_eq!(eval_expr("'{:#b}'.format(-16)").as_str(), Some("-0b10000"));
    assert_eq!(eval_expr("'{:o}'.format(8)").as_str(), Some("10"));
    assert_eq!(eval_expr("'{:#o}'.format(8)").as_str(), Some("0o10"));
    assert_eq!(eval_expr("'{:x}'.format(127)").as_str(), Some("7f"));
    assert_eq!(eval_expr("'{:#X}'.format(127)").as_str(), Some("0X7F"));
}

#[test]
fn test_format_floats() {
    assert_eq!(eval_expr("'{:f}'.format(3.14)").as_str(), Some("3.140000"));
    assert_eq!(eval_expr("'{:.2f}'.format(2.7)").as_str(), Some("2.70"));
    assert_eq!(eval_expr("'{:.2e}'.format(2.7)").as_str(), Some("2.70e+00"));
    assert_eq!(
        eval_expr("'{:.3E}'.format(2.7)").as_str(),
        Some("2.700E+00")
    );
    assert_eq!(
        eval_expr("'{:g}'.format(3.141593)").as_str(),
        Some("3.14159")
    );
    assert_eq!(
        eval_expr("'{:g}'.format(3141593)").as_str(),
        Some("3.14159e+06")
    );
    assert_eq!(
        eval_expr("'{:g}'.format(3141593)").as_str(),
        Some("3.14159e+06")
    );
    assert_eq!(
        eval_expr("'{:>10g}'.format(0.3141593)").as_str(),
        Some("  0.314159")
    );
    assert_eq!(
        eval_expr("'{:=<10g}'.format(0.3141593)").as_str(),
        Some("0.314159==")
    );
    assert_eq!(
        eval_expr("'{:+G}'.format(0.00003141593)").as_str(),
        Some("+3.14159E-05")
    );
    assert_eq!(
        eval_expr("'{:}'.format(0.00000004)").as_str(),
        Some("4e-08")
    );
    assert_eq!(eval_expr("'{}'.format(0.00000004)").as_str(), Some("4e-08"));
    assert_eq!(eval_expr("'{}'.format(0.0004)").as_str(), Some("0.0004"));
    assert_eq!(eval_expr("'{}'.format(100.00)").as_str(), Some("100.0"));
}

#[test]
fn test_format_string() {
    assert_eq!(
        eval_expr("'{:s} {:}!'.format('Hello', 'world')").as_str(),
        Some("Hello world!")
    );
    assert_eq!(
        eval_expr("'{0:<7s},{0:=>7}'.format('Hello')").as_str(),
        Some("Hello  ,==Hello")
    );
}

#[test]
fn test_format_field_name() {
    assert_eq!(
        eval_expr("'({0:d}, {1:d}, {2})'.format(1, 2, 3)").as_str(),
        Some("(1, 2, 3)")
    );
    assert_eq!(
        eval_expr("'({:}, {:d}, {:02d})'.format(1,2,3)").as_str(),
        Some("(1, 2, 03)")
    );
    assert!(eval_err_expr("'({1:d}, {0:d}, {})'.format(1, 2, 3)")
        .contains("cannot switch from manual field specification to automatic numbering"));
    assert!(eval_err_expr("'({:d}, {0:d}, {})'.format(1, 2, 3)")
        .contains("cannot switch from automatic numbering to manual field specification"));

    // test mix of indexed and keyed args
    assert_eq!(
        eval_expr("'({1:d},{0:d},{key:d})'.format(1, 2, key=42)").as_str(),
        Some("(2,1,42)")
    );

    // test attr access
    assert_eq!(
        eval_expr("'({l[1]}, {d[k]}, {d.l[0][k2]})'.format(l=[10, 11], d={'k':0, 'l':[{'k2':1}]})")
            .as_str(),
        Some("(11, 0, 1)")
    );
    assert_eq!(
        eval_expr("'({0[1]}, {1.k}, {1.l[0].k2})'.format([10, 11], {'k':0, 'l':[{'k2':1}]})")
            .as_str(),
        Some("(11, 0, 1)")
    );
}

#[test]
fn test_format_escape_sequence() {
    assert_eq!(
        eval_expr("'{0:d}:{{boo}}'.format(1)").as_str(),
        Some("1:{boo}")
    );
    assert_eq!(
        eval_expr("'{{{{ {} }}}} {{'.format(1)").as_str(),
        Some("{{ 1 }} {")
    );
    assert_eq!(eval_expr("'}} and }}'.format(1)").as_str(), Some("} and }"));
    assert!(
        eval_err_expr("'{'.format(1)").contains("missing closing '}' in format spec at offset 0")
    );
    assert!(eval_err_expr("'}'.format(1)")
        .contains("invalid single '}' in format string at offset 0; use escape sequence '}}'"));
}

#[test]
fn test_format_integral_grouping() {
    assert_eq!(eval_expr("'{:,}'.format(123)").as_str(), Some("123"));
    assert_eq!(eval_expr("'{:,}'.format(1234)").as_str(), Some("1,234"));
    assert_eq!(eval_expr("'{:_}'.format(1234)").as_str(), Some("1_234"));
    assert_eq!(eval_expr("'{:_}'.format(123456)").as_str(), Some("123_456"));
    assert_eq!(
        eval_expr("'{:,}'.format(10000000)").as_str(),
        Some("10,000,000")
    );
    assert_eq!(
        eval_expr("'{:010,}'.format(123456)").as_str(),
        Some("00,123,456")
    );
    assert_eq!(
        eval_expr("'{:08,}'.format(1234)").as_str(),
        Some("0,001,234")
    );
    assert_eq!(eval_expr("'{:04,}'.format(123)").as_str(), Some("0,123"));
    assert_eq!(eval_expr("'{:_b}'.format(42)").as_str(), Some("10_1010"));
    assert_eq!(
        eval_expr("'{:_b}'.format(0b11001001)").as_str(),
        Some("1100_1001")
    );
    assert_eq!(
        eval_expr("'{:_b}'.format(0b111001001)").as_str(),
        Some("1_1100_1001")
    );
    assert_eq!(
        eval_expr("'{:_b}'.format(0b1100111001001)").as_str(),
        Some("1_1001_1100_1001")
    );
    assert_eq!(
        eval_expr("'{:#_o}'.format(0o1776)").as_str(),
        Some("0o1776")
    );
    assert_eq!(
        eval_expr("'{:#_o}'.format(0o17765)").as_str(),
        Some("0o1_7765")
    );
    assert_eq!(
        eval_expr("'{:#010_o}'.format(0o17765)").as_str(),
        Some("0o001_7765")
    );
    assert_eq!(
        eval_expr("'{:_x}'.format(0x12ff0ef7)").as_str(),
        Some("12ff_0ef7")
    );
    assert_eq!(
        eval_expr("'{:#_x}'.format(0xfff12ff0ef7)").as_str(),
        Some("0xfff_12ff_0ef7")
    );
    assert_eq!(
        eval_expr("'{:#10_X}'.format(0x123456)").as_str(),
        Some(" 0X12_3456")
    );
    assert_eq!(
        eval_expr("'{:#010_X}'.format(0x123456)").as_str(),
        Some("0X012_3456")
    );
    assert_eq!(
        eval_expr("'{:#014_X}'.format(0x123456)").as_str(),
        Some("0X00_0012_3456")
    );
    assert_eq!(
        eval_expr("'{:,f}'.format(12345.6789)").as_str(),
        Some("12,345.678900")
    );
    assert_eq!(
        eval_expr("'{:_F}'.format(1234567.6789)").as_str(),
        Some("1_234_567.678900")
    );
    assert_eq!(
        eval_expr("'{:,e}'.format(1234567.6789)").as_str(),
        Some("1.234568e+06")
    );
    assert_eq!(
        eval_expr("'{:,.2e}'.format(123.45)").as_str(),
        Some("1.23e+02")
    );
    assert_eq!(
        eval_expr("'{:010,.2e}'.format(123.45)").as_str(),
        Some("001.23e+02")
    );
    assert_eq!(
        eval_expr("'{:011,.2e}'.format(123.45)").as_str(),
        Some("0,001.23e+02")
    );
    assert_eq!(
        eval_expr("'{:09,.0e}'.format(1)").as_str(),
        Some("0,001e+00")
    );
    assert_eq!(
        eval_expr("'{:09,.0E}'.format(1)").as_str(),
        Some("0,001E+00")
    );
    assert_eq!(
        eval_expr("'{:#09,.0E}'.format(1)").as_str(),
        Some("0,001.E+00")
    );
    assert_eq!(
        eval_expr("'{:_g}'.format(1234.56)").as_str(),
        Some("1_234.56")
    );
    assert_eq!(
        eval_expr("'{:09_g}'.format(1234.56)").as_str(),
        Some("01_234.56")
    );
    assert!(eval_err_expr("'{:,x}'.format(1)").contains("',' cannot be specified with hex format"));
}
