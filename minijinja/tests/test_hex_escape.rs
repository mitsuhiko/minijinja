use minijinja::Environment;

#[test]
fn test_hex_escape_sequences() {
    let env = Environment::new();
    
    // Test \x00 (null character)
    let result = env.render_str(r#"{{ "\x00" }}"#, ()).unwrap();
    assert_eq!(result, "\x00");
    
    // Test \x42 (letter 'B') 
    let result = env.render_str(r#"{{ "\x42" }}"#, ()).unwrap();
    assert_eq!(result, "B");
    
    // Test \x20 (space)
    let result = env.render_str(r#"{{ "hello\x20world" }}"#, ()).unwrap();
    assert_eq!(result, "hello world");
    
    // Test multiple hex sequences
    let result = env.render_str(r#"{{ "\x41\x42\x43" }}"#, ()).unwrap();
    assert_eq!(result, "ABC");
    
    // Test list context like in the issue description
    let result = env.render_str(r#"{{ ["\x00"] }}"#, ()).unwrap();
    assert_eq!(result, "[\"\\0\"]");
}