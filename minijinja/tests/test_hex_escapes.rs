use minijinja::{Environment, render};

#[test]
fn test_hex_escape_sequences() {
    let env = Environment::new();
    
    // Test basic hex escape sequences
    assert_eq!(
        env.render_str(r#"{{ "\x41\x42\x43" }}"#, ()).unwrap(),
        "ABC"
    );
    
    // Test null byte
    assert_eq!(
        env.render_str(r#"{{ "\x00" }}"#, ()).unwrap(),
        "\0"
    );
    
    // Test space character
    assert_eq!(
        env.render_str(r#"{{ "\x20" }}"#, ()).unwrap(),
        " "
    );
    
    // Test lowercase hex
    assert_eq!(
        env.render_str(r#"{{ "\xff" }}"#, ()).unwrap(),
        "\u{ff}"
    );
    
    // Test uppercase hex
    assert_eq!(
        env.render_str(r#"{{ "\xFF" }}"#, ()).unwrap(),
        "\u{FF}"
    );
    
    // Test mixed with other escape sequences
    assert_eq!(
        env.render_str(r#"{{ "Hello\x20\x57\x6f\x72\x6c\x64\x21" }}"#, ()).unwrap(),
        "Hello World!"
    );
    
    // Test with regular escapes
    assert_eq!(
        env.render_str(r#"{{ "Line1\nLine2\x20\x2d\x20Tab:\t\x41" }}"#, ()).unwrap(),
        "Line1\nLine2 - Tab:\tA"
    );
}

#[test]
fn test_octal_escape_sequences() {
    let env = Environment::new();
    
    // Test basic octal escape sequences
    assert_eq!(
        env.render_str(r#"{{ "\101\102\103" }}"#, ()).unwrap(),
        "ABC"
    );
    
    // Test null byte
    assert_eq!(
        env.render_str(r#"{{ "\0" }}"#, ()).unwrap(),
        "\0"
    );
    
    // Test space character (40 octal = 32 decimal)
    assert_eq!(
        env.render_str(r#"{{ "\40" }}"#, ()).unwrap(),
        " "
    );
    
    // Test maximum value (377 octal = 255 decimal)
    assert_eq!(
        env.render_str(r#"{{ "\377" }}"#, ()).unwrap(),
        "\u{ff}"
    );
    
    // Test 1, 2, and 3 digit sequences
    assert_eq!(
        env.render_str(r#"{{ "\7" }}"#, ()).unwrap(),
        "\u{07}"
    );
    assert_eq!(
        env.render_str(r#"{{ "\10" }}"#, ()).unwrap(),
        "\u{08}"
    );
    assert_eq!(
        env.render_str(r#"{{ "\101" }}"#, ()).unwrap(),
        "A"
    );
    
    // Test octal stops at non-octal digits
    assert_eq!(
        env.render_str(r#"{{ "\108" }}"#, ()).unwrap(),
        "\u{08}8"
    );
    
    // Test mixed with other characters
    assert_eq!(
        env.render_str(r#"{{ "Hello\40World\41" }}"#, ()).unwrap(),
        "Hello World!"
    );
}

#[test]
fn test_hex_escape_errors() {
    let env = Environment::new();
    
    // Test invalid hex sequences should error
    assert!(env.render_str(r#"{{ "\x" }}"#, ()).is_err());
    assert!(env.render_str(r#"{{ "\x1" }}"#, ()).is_err());
    assert!(env.render_str(r#"{{ "\xGG" }}"#, ()).is_err());
    assert!(env.render_str(r#"{{ "\xZ1" }}"#, ()).is_err());
}

#[test]
fn test_escape_sequences_in_expressions() {
    let env = Environment::new();
    
    // Test hex in string concatenation
    assert_eq!(
        env.render_str(r#"{{ "A" ~ "\x42" ~ "C" }}"#, ()).unwrap(),
        "ABC"
    );
    
    // Test octal in string concatenation
    assert_eq!(
        env.render_str(r#"{{ "A" ~ "\102" ~ "C" }}"#, ()).unwrap(),
        "ABC"
    );
    
    // Test in conditionals
    assert_eq!(
        env.render_str(r#"{% if "\x41" == "A" %}Hex Match{% endif %}"#, ()).unwrap(),
        "Hex Match"
    );
    
    assert_eq!(
        env.render_str(r#"{% if "\101" == "A" %}Octal Match{% endif %}"#, ()).unwrap(),
        "Octal Match"
    );
}

#[test]
fn test_mixed_escape_sequences() {
    let env = Environment::new();
    
    // Test mixing hex, octal, and regular escapes
    assert_eq!(
        env.render_str(r#"{{ "Mix:\x48\145\x6c\154\x6f\40\x57\157\x72\154\144\x21" }}"#, ()).unwrap(),
        "Mix:Hello World!"
    );
    
    // Test with line breaks and tabs
    assert_eq!(
        env.render_str(r#"{{ "Line1\nHex:\x41\40Octal:\101\tEnd" }}"#, ()).unwrap(),
        "Line1\nHex:A Octal:A\tEnd"
    );
}

#[test]
fn test_render_macro() {
    // Test hex with the render! macro
    assert_eq!(
        render!(r#"{{ "\x48\x65\x6c\x6c\x6f" }}"#),
        "Hello"
    );
    
    assert_eq!(
        render!(r#"{{ "\x57\x6f\x72\x6c\x64\x21" }}"#),
        "World!"
    );
    
    // Test octal with the render! macro
    assert_eq!(
        render!(r#"{{ "\110\145\154\154\157" }}"#),
        "Hello"
    );
    
    assert_eq!(
        render!(r#"{{ "\127\157\162\154\144\41" }}"#),
        "World!"
    );
}