use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_weak_upgrade_race");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    test_weak_reference_upgrade_race(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_weak_reference_upgrade_race(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Weak Reference Upgrade Race ===");
    println!("Trying to trigger a scenario where weak_handle.upgrade() fails in the file watcher callback");
    
    // This test will repeatedly create and drop AutoReloaders quickly
    // while simultaneously making file changes, trying to catch the moment
    // when the file watcher's weak reference can't be upgraded
    
    for iteration in 0..10 {
        println!("\n--- Iteration {} ---", iteration);
        
        // Create an AutoReloader that will have a very short lifetime
        let reloader = {
            let temp_dir = temp_dir.clone();
            Arc::new(AutoReloader::new(move |notifier| {
                let mut env = Environment::new();
                env.set_loader(path_loader(&temp_dir));
                notifier.watch_path(&temp_dir, true);
                println!("  Environment created in iteration {}", iteration);
                Ok(env)
            }))
        };
        
        // Get a notifier handle - this creates a weak reference
        let notifier = reloader.notifier();
        
        // Start a background thread that will make rapid file changes
        let template_path_clone = template_path.clone();
        let file_changer = thread::spawn(move || {
            for i in 0..5 {
                let content = format!("Race Version: {}-{} - {{{{name}}}}", iteration, i);
                fs::write(&template_path_clone, content).unwrap();
                thread::sleep(Duration::from_millis(10));
            }
        });
        
        // Use the reloader briefly
        {
            let env = reloader.acquire_env().unwrap();
            let tmpl = env.get_template("test.html").unwrap();
            let output = tmpl.render(context!(name => format!("Test-{}", iteration))).unwrap();
            println!("  Initial: {}", output);
        }
        
        // Now drop the reloader while file changes might still be happening
        // This could cause the weak reference in the file watcher callback to fail
        drop(reloader);
        
        // Wait a bit to let file events potentially fire with the now-invalid weak ref
        thread::sleep(Duration::from_millis(50));
        
        file_changer.join().unwrap();
        
        // Check if the notifier is now dead (indicating weak ref failure)
        if notifier.is_dead() {
            println!("  ✅ Notifier is dead (expected after dropping AutoReloader)");
        } else {
            println!("  ❌ Notifier is still alive (unexpected?)");
        }
    }
    
    println!("\n--- Final Test: Creating new AutoReloader after race ---");
    
    // Create a fresh AutoReloader and see if file watching still works
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
    
    // Make a final change
    fs::write(&template_path, "Final Race Test: FINAL - {{name}}").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = final_reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final")).unwrap();
    println!("Final result: {}", output);
    
    if output.contains("FINAL") {
        println!("✅ New AutoReloader works correctly after race test");
    } else {
        println!("❌ New AutoReloader failed after race test");
    }
}