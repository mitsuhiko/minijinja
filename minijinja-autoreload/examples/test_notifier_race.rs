use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    // Create a temporary directory for our test templates
    let temp_dir = std::env::temp_dir().join("minijinja_notifier_race_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Create initial template
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    println!("Template path: {:?}", template_path);
    
    test_notifier_race_condition(&temp_dir, &template_path);
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}

fn test_notifier_race_condition(temp_dir: &PathBuf, template_path: &PathBuf) {
    println!("\n=== Testing Notifier Race Condition ===");
    
    // Create an AutoReloader
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
    
    // Get a handle to the notifier BEFORE any file changes
    let notifier = reloader.notifier();
    
    // Thread 1: Constantly acquires and releases the environment
    let reloader1 = Arc::clone(&reloader);
    let handle1 = thread::spawn(move || {
        for i in 0..100 {
            let _env = reloader1.acquire_env().unwrap();
            if i % 20 == 0 {
                println!("Thread 1: Acquired environment at iteration {}", i);
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    
    // Thread 2: Makes rapid file changes
    let template_path2 = template_path.clone();
    let handle2 = thread::spawn(move || {
        for i in 0..20 {
            thread::sleep(Duration::from_millis(50));
            let content = format!("Rapid Version: {} - {{{{name}}}}", i + 2);
            fs::write(&template_path2, content).unwrap();
            println!("Thread 2: Made file change {}", i + 1);
        }
    });
    
    // Thread 3: Manually triggers reloads using the notifier
    let handle3 = thread::spawn(move || {
        for i in 0..30 {
            thread::sleep(Duration::from_millis(33));
            notifier.request_reload();
            if i % 10 == 0 {
                println!("Thread 3: Manual reload request {}", i);
            }
        }
    });
    
    // Thread 4: Also accesses the environment but with different timing
    let reloader4 = Arc::clone(&reloader);
    let handle4 = thread::spawn(move || {
        for i in 0..50 {
            thread::sleep(Duration::from_millis(20));
            match reloader4.acquire_env() {
                Ok(env) => {
                    if let Ok(tmpl) = env.get_template("test.html") {
                        if let Ok(output) = tmpl.render(context!(name => format!("T4-{}", i))) {
                            if i % 25 == 0 {
                                println!("Thread 4: {}", output);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Thread 4: Error at iteration {}: {}", i, e);
                }
            }
        }
    });
    
    // Wait for all threads to complete
    handle1.join().unwrap();
    handle2.join().unwrap();
    handle3.join().unwrap();
    handle4.join().unwrap();
    
    println!("\n--- Final test after race conditions ---");
    
    // Give some time for any pending file system events
    thread::sleep(Duration::from_millis(1000));
    
    // Make one final change
    fs::write(&template_path, "Final Race Version: FINAL - {{name}} (AFTER RACE)").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    // Test if autoreload still works
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final Race Test")).unwrap();
    println!("Final race test: {}", output);
    
    if output.contains("FINAL") && output.contains("AFTER RACE") {
        println!("✅ Autoreload survived the race conditions!");
    } else {
        println!("❌ Autoreload failed during race conditions");
        println!("   Expected to see 'FINAL' and 'AFTER RACE' in output");
    }
    
    // Also test that the notifier itself isn't dead
    if reloader.notifier().is_dead() {
        println!("❌ Notifier is marked as dead!");
    } else {
        println!("✅ Notifier is still alive");
    }
}