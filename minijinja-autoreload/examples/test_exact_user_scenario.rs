use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_exact_user_scenario");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    test_exact_user_scenario(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_exact_user_scenario(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Exact User Scenario ===");
    println!("Simulating: AutoReloader in Arc with multiple worker threads");
    println!("Pattern: Each worker acquires, copies environment, and drops guard quickly");
    
    // Create autoreloader as user described - wrapped in Arc for sharing
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Environment created/recreated");
            Ok(env)
        }
    }));
    
    // Start multiple worker threads like a web server would
    let handles: Vec<_> = (0..5).map(|worker_id| {
        let reloader = Arc::clone(&reloader);
        let template_path = template_path.clone();
        
        thread::spawn(move || {
            for i in 0..20 {
                // User's pattern: acquire environment when acquiring, copy it
                let guard = reloader.acquire_env().unwrap();
                
                // User mentioned "copying" the environment
                // This simulates keeping a reference to the environment
                let env = guard.clone(); // This clones the Environment itself
                drop(guard); // Drop the guard immediately as user might do
                
                // Use the copied environment
                let tmpl = env.get_template("test.html").unwrap();
                let output = tmpl.render(context!(name => format!("W{}-{}", worker_id, i))).unwrap();
                
                if i % 10 == 0 {
                    println!("Worker {}, iteration {}: {}", worker_id, i, output);
                }
                
                // Worker 0 makes a template change
                if worker_id == 0 && i == 8 {
                    println!("Worker {} making template change...", worker_id);
                    fs::write(&template_path, "Version: 2 - {{name}} (WORKER UPDATE)").unwrap();
                }
                
                thread::sleep(Duration::from_millis(100));
            }
            println!("Worker {} completed", worker_id);
        })
    }).collect();
    
    // Let the workers run for a bit, then make an external change
    thread::sleep(Duration::from_millis(1000));
    println!("\nMaking external template change...");
    fs::write(&template_path, "Version: 3 - {{name}} (EXTERNAL UPDATE)").unwrap();
    
    // Wait for all workers to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Give time for file watcher to process the external change
    thread::sleep(Duration::from_millis(1000));
    
    // Final test - see if the autoreloader still detects changes
    println!("\n--- Final Test: Is autoreload still working? ---");
    
    // Make one more change
    fs::write(&template_path, "Version: FINAL - {{name}} (FINAL TEST)").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    // Test if it's detected
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final Test")).unwrap();
    println!("Final test result: {}", output);
    
    if output.contains("FINAL TEST") {
        println!("✅ SUCCESS: Autoreload is still working after multithreaded usage!");
    } else {
        println!("❌ FAILURE: Autoreload stopped working - this reproduces the user's issue!");
        println!("   Expected to see 'FINAL TEST' but got older template");
    }
    
    // Additional test: Check if notifier is dead
    let notifier = reloader.notifier();
    if notifier.is_dead() {
        println!("❌ ADDITIONAL ISSUE: Notifier is marked as dead!");
    } else {
        println!("✅ Notifier is still alive");
    }
}