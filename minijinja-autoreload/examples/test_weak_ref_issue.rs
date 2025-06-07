use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_weak_ref_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    println!("Template path: {:?}", template_path);
    
    test_weak_reference_issue(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_weak_reference_issue(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Weak Reference Issue ===");
    
    // This test simulates a scenario where the AutoReloader might be 
    // temporarily without strong references, causing the file watcher's
    // weak reference to fail
    
    let template_path_clone = template_path.clone();
    let temp_dir_clone = temp_dir.clone();
    
    // Thread that will create and drop AutoReloaders rapidly
    let handle = thread::spawn(move || {
        for iteration in 0..10 {
            println!("Creating AutoReloader iteration {}", iteration);
            
            // Create an AutoReloader
            let reloader = AutoReloader::new({
                let temp_dir = temp_dir_clone.clone();
                move |notifier| {
                    let mut env = Environment::new();
                    env.set_loader(path_loader(&temp_dir));
                    notifier.watch_path(&temp_dir, true);
                    println!("Environment created in iteration {}", iteration);
                    Ok(env)
                }
            });
            
            // Use it briefly
            let env = reloader.acquire_env().unwrap();
            let tmpl = env.get_template("test.html").unwrap();
            let output = tmpl.render(context!(name => format!("Iter-{}", iteration))).unwrap();
            println!("Iteration {}: {}", iteration, output);
            drop(env);
            
            // Make a file change
            if iteration == 3 {
                println!("Making file change during iteration {}", iteration);
                fs::write(&template_path_clone, format!("Version: {} - {{{{name}}}}", iteration + 2)).unwrap();
            }
            
            // Drop the reloader (this might cause the file watcher's weak ref to become invalid)
            drop(reloader);
            
            // Sleep to let file watcher potentially process events
            thread::sleep(Duration::from_millis(200));
        }
    });
    
    // Wait for the test to complete
    handle.join().unwrap();
    
    println!("\n--- Testing if file watcher is still active after drops ---");
    
    // Now create a new AutoReloader and see if it detects changes
    let reloader = AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Final environment created");
            Ok(env)
        }
    });
    
    // Make a change
    fs::write(&template_path, "Version: FINAL - {{name}} (AFTER DROPS)").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    // Check if it's detected
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final Test")).unwrap();
    println!("Final test: {}", output);
    
    if output.contains("FINAL") {
        println!("✅ File watcher still working after multiple drops!");
    } else {
        println!("❌ File watcher stopped working after drops");
    }
}