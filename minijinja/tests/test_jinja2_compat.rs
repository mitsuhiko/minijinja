use minijinja::Environment;

#[test]
fn test_boolean_rendering() {
    let env = Environment::new();
    
    // Test boolean rendering in arrays/debug context
    assert_eq!(
        env.render_str("{{ [true, false] }}", ()).unwrap(),
        "[True, False]"  // Should be Python-style capitalization
    );
    
    // Test boolean rendering in display context 
    assert_eq!(
        env.render_str("{{ true }}", ()).unwrap(),
        "True"
    );
    assert_eq!(
        env.render_str("{{ false }}", ()).unwrap(),
        "False"
    );
}

#[test]
fn test_none_rendering() {
    let env = Environment::new();
    
    // Test none rendering in arrays/debug context
    assert_eq!(
        env.render_str("{{ [none] }}", ()).unwrap(),
        "[None]"  // Should be Python-style capitalization
    );
    
    // Test none rendering in display context (should still be empty)
    assert_eq!(
        env.render_str("{{ none }}", ()).unwrap(),
        ""  // Display should still be empty
    );
}

#[test]
fn test_string_quoting() {
    let env = Environment::new();
    
    // Test smart string quoting
    let result = env.render_str(r#"{{ ['foo', "bar'baz", '\x13'] }}"#, ()).unwrap();
    
    // Should prefer single quotes, use double quotes when string contains single quotes
    // Note: \x13 should render the actual character or appropriate escape
    assert!(result.contains("'foo'"));  // Simple string with single quotes
    assert!(result.contains("\"bar'baz\""));  // String with single quote, use double quotes
    
    // Test that escape sequences work properly in the display
    let result2 = env.render_str(r#"{{ ['normal', 'with\'quote', "with\"quote"] }}"#, ()).unwrap();
    println!("String quoting result: {}", result2);
}

#[test]
fn test_mixed_types_rendering() {
    let env = Environment::new();
    
    // Test mixed types in an array
    let result = env.render_str("{{ [42, true, none, 'hello', false] }}", ()).unwrap();
    println!("Mixed types result: {}", result);
    
    // Should show Python-style formatting
    assert!(result.contains("True"));
    assert!(result.contains("False"));
    assert!(result.contains("None"));
    assert!(result.contains("'hello'"));
}