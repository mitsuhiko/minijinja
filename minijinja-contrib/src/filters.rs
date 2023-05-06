#[cfg(test)]
use similar_asserts::assert_eq;
use std::convert::TryFrom;

use minijinja::value::Value;
use minijinja::{Error, ErrorKind};

/// Returns a plural suffix if the value is not 1, '1', or an object of
/// length 1.
///
/// By default, the plural suffix is 's' and the singular suffix is
/// empty (''). You can specify a singular suffix as the first argument (or
/// `None`, for the default). You can specify a plural suffix as the second
/// argument (or `None`, for the default).
///
/// ```jinja
/// {{ users|length }} user{{ users|pluralize }}.
/// ```
///
/// ```jinja
/// {{ entities|length }} entit{{ entities|pluralize("y", "ies") }}.
/// ```
///
/// ```jinja
/// {{ platypuses|length }} platypus{{ platypuses|pluralize(None, "es") }}.
/// ```
pub fn pluralize(v: Value, singular: Option<Value>, plural: Option<Value>) -> Result<Value, Error> {
    let is_singular = match v.len() {
        Some(val) => val == 1,
        None => match i64::try_from(v.clone()) {
            Ok(val) => val == 1,
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!(
                        "Pluralize argument is not an integer, or a sequence / object with a \
                         length but of type {}",
                        v.kind()
                    ),
                ));
            }
        },
    };

    let (rv, default) = if is_singular {
        (singular.unwrap_or(Value::UNDEFINED), "")
    } else {
        (plural.unwrap_or(Value::UNDEFINED), "s")
    };

    if rv.is_undefined() || rv.is_none() {
        Ok(Value::from(default))
    } else {
        Ok(rv)
    }
}

#[test]
fn test_pluralize() {
    use minijinja::context;

    let mut env = minijinja::Environment::new();

    env.add_filter("pluralize", pluralize);
    for (num, s) in [
        (0, "You have 0 messages."),
        (1, "You have 1 message."),
        (10, "You have 10 messages."),
    ] {
        assert_eq!(
            env.render_str(
                "You have {{ num_messages }} message{{ num_messages|pluralize }}.",
                context! {
                    num_messages => num,
                }
            )
            .unwrap(),
            s
        );
    }

    for (num, s) in [
        (0, "You have 0 walruses."),
        (1, "You have 1 walrus."),
        (10, "You have 10 walruses."),
    ] {
        assert_eq!(
            env.render_str(
                r#"You have {{ num_walruses }} walrus{{ num_walruses|pluralize(None, "es") }}."#,
                context! {
                    num_walruses => num,
                }
            )
            .unwrap(),
            s
        );
    }

    for (num, s) in [
        (0, "You have 0 cherries."),
        (1, "You have 1 cherry."),
        (10, "You have 10 cherries."),
    ] {
        assert_eq!(
            env.render_str(
                r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
                context! {
                    num_cherries => num,
                }
            )
            .unwrap(),
            s
        );
    }

    assert_eq!(
        env.render_str(
            r#"You have {{ num_cherries|length }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
            context! {
                num_cherries => vec![(); 5],
            }
        )
        .unwrap(),
        "You have 5 cherries."
    );
    assert_eq!(
        env.render_str(
            r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
            context! {
                num_cherries => 5,
            }
        )
        .unwrap(),
        "You have 5 cherries."
    );
    assert_eq!(
        env.render_str(
            r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
            context! {
                num_cherries => 0.5f32,
            }
        )
        .unwrap_err()
        .to_string(),
        "invalid operation: Pluralize argument is not an integer, or a sequence / object with \
            a length but of type number (in <string>:1)",
    );
}
