use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_fs_watcher_take_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    test_fs_watcher_take_issue(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_fs_watcher_take_issue(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing FS Watcher Take Issue ===");
    println!("This test demonstrates the issue where file watching stops after the first reload");
    
    // Create autoreloader with default settings (no persistent watcher, no fast reload)
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            // Note: NOT calling notifier.persistent_watch(true) or notifier.set_fast_reload(true)
            println!("Environment created - file watcher should be destroyed after this reload");
            Ok(env)
        }
    }));
    
    // Step 1: Initial state
    println!("\n1. Testing initial state:");
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Initial")).unwrap();
    println!("   {}", output);
    drop(env);
    
    // Step 2: Make first change - this should trigger reload and destroy the file watcher
    println!("\n2. Making first file change (this will trigger reload and destroy watcher):");
    fs::write(&template_path, "Version: 2 - {{name}} (FIRST CHANGE)").unwrap();
    thread::sleep(Duration::from_millis(500)); // Give file watcher time to detect
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "After First Change")).unwrap();
    println!("   {}", output);
    drop(env);
    
    // Step 3: Make second change - this should NOT be detected because watcher was destroyed
    println!("\n3. Making second file change (this should NOT be detected):");
    fs::write(&template_path, "Version: 3 - {{name}} (SECOND CHANGE - SHOULD NOT BE SEEN)").unwrap();
    thread::sleep(Duration::from_millis(500)); // Give time for detection
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "After Second Change")).unwrap();
    println!("   {}", output);
    drop(env);
    
    // Analysis
    if output.contains("SECOND CHANGE") {
        println!("\n❌ UNEXPECTED: Second change was detected! (Test may be flawed)");
    } else {
        println!("\n✅ CONFIRMED: Second change was NOT detected!");
        println!("   This demonstrates the issue - file watcher stops working after first reload");
        println!("   The template still shows 'FIRST CHANGE' instead of 'SECOND CHANGE'");
    }
    
    println!("\n--- Testing the fix ---");
    test_with_persistent_watcher(temp_dir, template_path);
}

fn test_with_persistent_watcher(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Fix: Persistent Watcher ===");
    
    // Reset template
    fs::write(&template_path, "Fix Version: 1 - {{name}}").unwrap();
    
    // Create autoreloader with persistent watcher enabled
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            notifier.persistent_watch(true); // This is the fix!
            println!("Environment created with persistent watcher");
            Ok(env)
        }
    }));
    
    // Step 1: Initial state
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Initial")).unwrap();
    println!("1. {}", output);
    drop(env);
    
    // Step 2: First change
    fs::write(&template_path, "Fix Version: 2 - {{name}} (FIRST CHANGE)").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "After First Change")).unwrap();
    println!("2. {}", output);
    drop(env);
    
    // Step 3: Second change - this SHOULD be detected with persistent watcher
    fs::write(&template_path, "Fix Version: 3 - {{name}} (SECOND CHANGE - SHOULD BE SEEN)").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "After Second Change")).unwrap();
    println!("3. {}", output);
    drop(env);
    
    // Analysis
    if output.contains("SECOND CHANGE") {
        println!("\n✅ SUCCESS: With persistent watcher, second change WAS detected!");
        println!("   This confirms that persistent_watch(true) fixes the issue");
    } else {
        println!("\n❌ FAILED: Even with persistent watcher, second change was not detected");
    }
}