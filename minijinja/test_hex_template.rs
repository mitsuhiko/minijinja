use minijinja::Environment;

fn main() {
    let env = Environment::new();
    
    println!("Testing hexadecimal escape sequences in templates:");
    
    // Test \x00 (null character)
    match env.render_str(r#"{{ "\x00" }}"#, ()) {
        Ok(result) => println!("\\x00: {:?} (length: {})", result, result.len()),
        Err(e) => println!("Error with \\x00: {}", e),
    }
    
    // Test \x42 (letter 'B')
    match env.render_str(r#"{{ "\x42" }}"#, ()) {
        Ok(result) => println!("\\x42: {:?}", result),
        Err(e) => println!("Error with \\x42: {}", e),
    }
    
    // Test \x20 (space)
    match env.render_str(r#"{{ "hello\x20world" }}"#, ()) {
        Ok(result) => println!("hello\\x20world: {:?}", result),
        Err(e) => println!("Error with hello\\x20world: {}", e),
    }
    
    // Test multiple hex sequences
    match env.render_str(r#"{{ "\x41\x42\x43" }}"#, ()) {
        Ok(result) => println!("\\x41\\x42\\x43: {:?}", result),
        Err(e) => println!("Error with \\x41\\x42\\x43: {}", e),
    }
    
    // Test list context like in the issue description
    match env.render_str(r#"{{ ["\x00"] }}"#, ()) {
        Ok(result) => println!("[\"\\x00\"]: {:?}", result),
        Err(e) => println!("Error with [\"\\x00\"]: {}", e),
    }
}