use minijinja_contrib::filters::pluralize;
#[cfg(test)]
use similar_asserts::assert_eq;

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
