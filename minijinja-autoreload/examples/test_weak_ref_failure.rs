use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_weak_ref_failure_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    test_weak_reference_failure(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_weak_reference_failure(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Weak Reference Failure Scenario ===");
    println!("Attempting to trigger the scenario where weak_notifier.watch_path() fails silently");
    
    // This simulates the user's Arc<AutoReloader> pattern but with rapid ref counting changes
    for attempt in 0..20 {
        println!("\nAttempt {}: Creating AutoReloader with potential ref count issues", attempt);
        
        // Create the autoreloader in a way that could cause reference counting issues
        let env_result = {
            let reloader = Arc::new(AutoReloader::new({
                let temp_dir = temp_dir.clone();
                move |notifier| {
                    println!("  Environment creator called");
                    
                    let mut env = Environment::new();
                    env.set_loader(path_loader(&temp_dir));
                    
                    // This is where the weak reference might fail
                    println!("  Calling watch_path - this might silently fail if weak ref is dead");
                    notifier.watch_path(&temp_dir, true);
                    
                    // Check if the notifier is dead (which would indicate weak ref failure)
                    if notifier.is_dead() {
                        println!("  ❌ CRITICAL: Notifier is dead during environment creation!");
                        println!("     This means watch_path() probably failed silently!");
                    } else {
                        println!("  ✅ Notifier is alive during environment creation");
                    }
                    
                    Ok(env)
                }
            }));
            
            // Use the reloader briefly, then drop our reference
            // This simulates a scenario where the Arc<AutoReloader> might be
            // temporarily without strong references
            let env_guard = reloader.acquire_env().unwrap();
            let result = env_guard.get_template("test.html").unwrap()
                .render(context!(name => format!("Attempt-{}", attempt))).unwrap();
            
            // Drop everything quickly
            drop(env_guard);
            drop(reloader);
            
            result
        };
        
        println!("  Result: {}", env_result);
        
        // Make a file change and see if it would be detected by a new reloader
        fs::write(&template_path, format!("Version: {} - {{{{name}}}}", attempt + 2)).unwrap();
        thread::sleep(Duration::from_millis(50));
    }
    
    println!("\n--- Final test: Create new reloader and check if file watching works ---");
    
    let final_reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Final environment created");
            Ok(env)
        }
    }));
    
    // Test if file watching works
    fs::write(&template_path, "Final Version: FINAL - {{name}}").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = final_reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final")).unwrap();
    println!("Final result: {}", output);
    
    if output.contains("FINAL") {
        println!("✅ File watching works in final test");
    } else {
        println!("❌ File watching failed in final test");
    }
}