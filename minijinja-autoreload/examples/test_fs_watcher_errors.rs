use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    println!("=== Testing File System Watcher Error Scenarios ===");
    
    test_nonexistent_directory();
    test_permission_issues();
    test_too_many_watchers();
    test_rapid_directory_creation_deletion();
}

fn test_nonexistent_directory() {
    println!("\n--- Test 1: Watching Non-existent Directory ---");
    
    let nonexistent_dir = PathBuf::from("/tmp/nonexistent_dir_12345");
    
    let reloader = AutoReloader::new({
        let nonexistent_dir = nonexistent_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&nonexistent_dir));
            
            println!("Attempting to watch non-existent directory: {:?}", nonexistent_dir);
            notifier.watch_path(&nonexistent_dir, true);
            
            Ok(env)
        }
    });
    
    // Try to use it
    match reloader.acquire_env() {
        Ok(_) => println!("✅ Succeeded (but watch probably failed silently)"),
        Err(e) => println!("❌ Failed: {}", e),
    }
    
    thread::sleep(Duration::from_millis(100));
}

fn test_permission_issues() {
    println!("\n--- Test 2: Permission Issues ---");
    
    // Try to watch a system directory that might have permission issues
    let system_dir = PathBuf::from("/etc");
    
    let reloader = AutoReloader::new({
        let system_dir = system_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            // Don't set a loader since /etc doesn't contain templates
            
            println!("Attempting to watch system directory: {:?}", system_dir);
            notifier.watch_path(&system_dir, true);
            
            // Create a simple template directly
            env.add_template("test", "Permission test: {{name}}").unwrap();
            Ok(env)
        }
    });
    
    match reloader.acquire_env() {
        Ok(env) => {
            let tmpl = env.get_template("test").unwrap();
            let output = tmpl.render(context!(name => "Test")).unwrap();
            println!("✅ Environment works: {}", output);
        },
        Err(e) => println!("❌ Failed: {}", e),
    }
    
    thread::sleep(Duration::from_millis(100));
}

fn test_too_many_watchers() {
    println!("\n--- Test 3: Creating Many Watchers ---");
    
    // Try to create many watchers to potentially hit system limits
    let temp_dir = std::env::temp_dir().join("minijinja_many_watchers");
    fs::create_dir_all(&temp_dir).unwrap();
    
    let mut reloaders = Vec::new();
    
    for i in 0..50 {
        let sub_dir = temp_dir.join(format!("subdir_{}", i));
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("test.html"), format!("Template {} - {{{{name}}}}", i)).unwrap();
        
        let reloader = AutoReloader::new({
            let sub_dir = sub_dir.clone();
            move |notifier| {
                let mut env = Environment::new();
                env.set_loader(path_loader(&sub_dir));
                notifier.watch_path(&sub_dir, true);
                
                if i % 10 == 0 {
                    println!("Created watcher {} for {:?}", i, sub_dir);
                }
                
                Ok(env)
            }
        });
        
        // Test the reloader
        match reloader.acquire_env() {
            Ok(_) => {
                if i % 10 == 0 {
                    println!("✅ Reloader {} works", i);
                }
            },
            Err(e) => {
                println!("❌ Reloader {} failed: {}", i, e);
                break;
            }
        }
        
        reloaders.push(reloader);
    }
    
    println!("Created {} watchers total", reloaders.len());
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
    thread::sleep(Duration::from_millis(100));
}

fn test_rapid_directory_creation_deletion() {
    println!("\n--- Test 4: Rapid Directory Creation/Deletion ---");
    
    let base_dir = std::env::temp_dir().join("minijinja_rapid_test");
    
    for i in 0..10 {
        let test_dir = base_dir.join(format!("rapid_{}", i));
        
        // Create directory and template
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(test_dir.join("test.html"), format!("Rapid {} - {{{{name}}}}", i)).unwrap();
        
        // Create watcher
        let reloader = AutoReloader::new({
            let test_dir = test_dir.clone();
            move |notifier| {
                let mut env = Environment::new();
                env.set_loader(path_loader(&test_dir));
                notifier.watch_path(&test_dir, true);
                Ok(env)
            }
        });
        
        // Use it briefly
        let env = reloader.acquire_env().unwrap();
        let tmpl = env.get_template("test.html").unwrap();
        let output = tmpl.render(context!(name => format!("Test-{}", i))).unwrap();
        if i % 3 == 0 {
            println!("Rapid test {}: {}", i, output);
        }
        drop(env);
        
        // Immediately delete the directory
        fs::remove_dir_all(&test_dir).unwrap();
        
        // This might cause file watcher errors
        thread::sleep(Duration::from_millis(10));
    }
    
    println!("✅ Rapid creation/deletion test completed");
}