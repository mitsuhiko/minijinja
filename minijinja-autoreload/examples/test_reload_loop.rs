use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_reload_loop_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    test_potential_reload_loop(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_potential_reload_loop(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Potential Reload Loop ===");
    
    use std::sync::atomic::{AtomicUsize, Ordering};
    let reload_count = Arc::new(AtomicUsize::new(0));
    let max_reloads = 20;
    
    // Create an AutoReloader with a counter
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        let reload_count = Arc::clone(&reload_count);
        move |notifier| {
            let count = reload_count.fetch_add(1, Ordering::SeqCst) + 1;
            println!("Environment created/recreated - Count: {}", count);
            
            if count > max_reloads {
                panic!("Too many reloads! Potential infinite loop detected.");
            }
            
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            Ok(env)
        }
    }));
    
    // Get notifier before starting
    let notifier = reloader.notifier();
    
    // Make a few rapid file changes while also manually requesting reloads
    for i in 0..5 {
        println!("Iteration {}: Making file change", i);
        fs::write(&template_path, format!("Loop Version: {} - {{{{name}}}}", i + 2)).unwrap();
        
        // Also manually request reload
        notifier.request_reload();
        
        // Try to acquire environment
        match reloader.acquire_env() {
            Ok(env) => {
                if let Ok(tmpl) = env.get_template("test.html") {
                    if let Ok(output) = tmpl.render(context!(name => format!("Test-{}", i))) {
                        println!("Iteration {}: {}", i, output);
                    }
                }
            }
            Err(e) => {
                println!("Error at iteration {}: {}", i, e);
            }
        }
        
        thread::sleep(Duration::from_millis(100));
    }
    
    println!("✅ Test completed without infinite loop");
}