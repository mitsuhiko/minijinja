use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_mutex_race_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    test_mutex_race_condition(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_mutex_race_condition(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Potential Mutex Race Condition ===");
    println!("Looking for race between AutoReloader.cached_env mutex and NotifierImpl mutex");
    
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Environment created - file watcher active");
            Ok(env)
        }
    }));
    
    // Thread 1: Constantly acquires and holds the environment for a while
    let reloader1 = Arc::clone(&reloader);
    let handle1 = thread::spawn(move || {
        for i in 0..20 {
            let env_guard = reloader1.acquire_env().unwrap();
            println!("Thread 1: Acquired environment (holding mutex), iteration {}", i);
            
            // Hold the environment guard (and thus the mutex) for a while
            thread::sleep(Duration::from_millis(100));
            
            // Use the environment
            let tmpl = env_guard.get_template("test.html").unwrap();
            let output = tmpl.render(context!(name => format!("T1-{}", i))).unwrap();
            if i % 5 == 0 {
                println!("Thread 1: {}", output);
            }
            
            // Environment guard drops here, releasing mutex
            drop(env_guard);
            thread::sleep(Duration::from_millis(50));
        }
    });
    
    // Thread 2: Makes rapid file changes while Thread 1 holds the mutex
    let template_path2 = template_path.clone();
    let handle2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25)); // Let thread 1 start first
        
        for i in 0..10 {
            let content = format!("Mutex Race Version: {} - {{{{name}}}}", i + 2);
            fs::write(&template_path2, content).unwrap();
            println!("Thread 2: Made file change {} (while T1 might hold mutex)", i + 1);
            thread::sleep(Duration::from_millis(150)); // Change files while T1 holds mutex
        }
    });
    
    // Thread 3: Tries to acquire environment immediately after changes
    let reloader3 = Arc::clone(&reloader);
    let handle3 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50)); // Start after some changes
        
        for i in 0..15 {
            let env_guard = reloader3.acquire_env().unwrap();
            let tmpl = env_guard.get_template("test.html").unwrap();
            let output = tmpl.render(context!(name => format!("T3-{}", i))).unwrap();
            if i % 5 == 0 {
                println!("Thread 3: {}", output);
            }
            drop(env_guard);
            thread::sleep(Duration::from_millis(100));
        }
    });
    
    // Wait for all threads
    handle1.join().unwrap();
    handle2.join().unwrap();
    handle3.join().unwrap();
    
    // Final verification
    println!("\n--- Final State Check ---");
    thread::sleep(Duration::from_millis(500));
    
    fs::write(&template_path, "Final Mutex Test: FINAL - {{name}}").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final")).unwrap();
    println!("Final result: {}", output);
    
    if output.contains("FINAL") {
        println!("✅ File watching still works after mutex contention");
    } else {
        println!("❌ File watching failed after mutex contention");
    }
}