use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_autoreload_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    println!("Template path: {:?}", template_path);
    
    // Test case 1: User's problematic pattern - AutoReloader in Arc
    test_autoreloader_in_arc(&temp_dir, &template_path);
    
    // Test case 2: Stress test with many concurrent accesses
    test_stress_concurrent_access(&temp_dir, &template_path);
    
    // Test case 3: Test with copied environments (another user pattern)
    test_copied_environments(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_autoreloader_in_arc(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing AutoReloader wrapped in Arc ===");
    
    // Create the autoreloader
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
    
    // Simulate multiple worker threads accessing the autoreloader
    let handles: Vec<_> = (0..3).map(|worker_id| {
        let reloader = Arc::clone(&reloader);
        let template_path = template_path.clone();
        
        thread::spawn(move || {
            for i in 0..10 {
                // Each worker acquires the environment
                let env = reloader.acquire_env().unwrap();
                let tmpl = env.get_template("test.html").unwrap();
                let output = tmpl.render(context!(name => format!("Worker-{}-{}", worker_id, i))).unwrap();
                println!("Worker {}, iteration {}: {}", worker_id, i, output);
                
                // Worker 0 modifies the template after a few iterations
                if worker_id == 0 && i == 3 {
                    println!("Worker {} updating template...", worker_id);
                    fs::write(&template_path, "Version: 2 - {{name}} (UPDATED)").unwrap();
                }
                
                // Drop the environment guard explicitly
                drop(env);
                
                thread::sleep(Duration::from_millis(200));
            }
        })
    }).collect();
    
    // Wait for all workers to complete
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Test if the autoreloader is still working after multithreaded access
    println!("\n--- Testing autoreloader after multithreaded access ---");
    
    // Make another change
    fs::write(&template_path, "Version: 3 - {{name}} (FINAL UPDATE)").unwrap();
    thread::sleep(Duration::from_millis(500)); // Give file watcher time to detect change
    
    // Check if the change is detected
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final Test")).unwrap();
    println!("Final test: {}", output);
    
    if output.contains("Version: 3") {
        println!("✅ Autoreload is still working!");
    } else {
        println!("❌ Autoreload appears to have stopped working - still shows old template");
    }
}

fn test_stress_concurrent_access(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Stress Testing with Many Concurrent Accesses ===");
    
    // Reset template
    fs::write(&template_path, "Stress Version: 1 - {{name}}").unwrap();
    
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Stress test environment created/recreated");
            Ok(env)
        }
    }));
    
    // Create many threads that constantly access the autoreloader
    let handles: Vec<_> = (0..10).map(|worker_id| {
        let reloader = Arc::clone(&reloader);
        let template_path = template_path.clone();
        
        thread::spawn(move || {
            for i in 0..50 {
                // Try to acquire environment - this might fail if there are race conditions
                match reloader.acquire_env() {
                    Ok(env) => {
                        if let Ok(tmpl) = env.get_template("test.html") {
                            if let Ok(output) = tmpl.render(context!(name => format!("W{}-{}", worker_id, i))) {
                                if i % 10 == 0 {
                                    println!("Worker {}, iteration {}: {}", worker_id, i, output);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("Worker {} failed to acquire env at iteration {}: {}", worker_id, i, e);
                    }
                }
                
                // One worker makes frequent changes
                if worker_id == 0 && i % 15 == 5 {
                    println!("Worker {} making change at iteration {}...", worker_id, i);
                    fs::write(&template_path, format!("Stress Version: {} - {{{{name}}}}", i / 15 + 2)).unwrap();
                }
                
                thread::sleep(Duration::from_millis(50));
            }
        })
    }).collect();
    
    // Wait for completion
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Final test
    thread::sleep(Duration::from_millis(500));
    fs::write(&template_path, "Stress Version: FINAL - {{name}}").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final Stress Test")).unwrap();
    println!("Final stress test: {}", output);
    
    if output.contains("FINAL") {
        println!("✅ Stress test passed - autoreload still working!");
    } else {
        println!("❌ Stress test failed - autoreload stopped working");
    }
}

fn test_copied_environments(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing User Pattern: Copying Environments ===");
    
    // Reset template
    fs::write(&template_path, "Copied Version: 1 - {{name}}").unwrap();
    
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Copied test environment created/recreated");
            Ok(env)
        }
    }));
    
    // Simulate the user's pattern of copying environments
    let handles: Vec<_> = (0..5).map(|worker_id| {
        let reloader = Arc::clone(&reloader);
        let template_path = template_path.clone();
        
        thread::spawn(move || {
            for i in 0..20 {
                // User's pattern: acquire and then "copy" the environment
                let guard = reloader.acquire_env().unwrap();
                
                // This simulates what the user might do - keep a reference longer
                let env_copy = guard.clone(); // This should clone the Environment inside
                drop(guard); // Drop the guard immediately
                
                // Use the "copied" environment
                if let Ok(tmpl) = env_copy.get_template("test.html") {
                    if let Ok(output) = tmpl.render(context!(name => format!("Copy-W{}-{}", worker_id, i))) {
                        if i % 5 == 0 {
                            println!("Copied Worker {}, iteration {}: {}", worker_id, i, output);
                        }
                    }
                }
                
                // Worker 0 makes changes
                if worker_id == 0 && i == 10 {
                    println!("Copied Worker {} updating template...", worker_id);
                    fs::write(&template_path, "Copied Version: 2 - {{name}} (UPDATED BY COPY TEST)").unwrap();
                }
                
                thread::sleep(Duration::from_millis(100));
            }
        })
    }).collect();
    
    // Wait for completion
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Final test to see if changes are still detected
    thread::sleep(Duration::from_millis(500));
    fs::write(&template_path, "Copied Version: FINAL - {{name}} (FINAL COPY UPDATE)").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final Copy Test")).unwrap();
    println!("Final copy test: {}", output);
    
    if output.contains("FINAL COPY UPDATE") {
        println!("✅ Copy test passed - autoreload still working!");
    } else {
        println!("❌ Copy test failed - autoreload stopped working");
    }
}