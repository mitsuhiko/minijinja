use minijinja::{context, Environment};
use minijinja_contrib::filters::pluralize;
use similar_asserts::assert_eq;

#[test]
fn test_pluralize() {
    let mut env = Environment::new();

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

#[test]
#[cfg(feature = "rand")]
fn test_random() {
    use minijinja::render;
    use minijinja_contrib::filters::random;

    let mut env = Environment::new();
    env.add_filter("random", random);

    insta::assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ [1, 2, 3, 4]|random }}"), @"2");
    insta::assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ 'HelloWorld'|random }}"), @"e");
}
