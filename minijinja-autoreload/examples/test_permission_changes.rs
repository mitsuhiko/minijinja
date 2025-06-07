use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use std::os::unix::fs::PermissionsExt;
use minijinja::{path_loader, Environment, context};
use minijinja_autoreload::AutoReloader;

fn main() {
    println!("=== Testing Permission Changes During File Watching ===");
    
    test_permission_changes_during_watch();
}

fn test_permission_changes_during_watch() {
    println!("\n--- Testing Permission Changes During Watch ---");
    
    let temp_dir = std::env::temp_dir().join("minijinja_permission_test");
    fs::create_dir_all(&temp_dir).unwrap();
    
    let template_path = temp_dir.join("test.html");
    fs::write(&template_path, "Version: 1 - {{name}}").unwrap();
    
    println!("Test directory: {:?}", temp_dir);
    
    // Create autoreloader
    let reloader = Arc::new(AutoReloader::new({
        let temp_dir = temp_dir.clone();
        move |notifier| {
            let mut env = Environment::new();
            env.set_loader(path_loader(&temp_dir));
            notifier.watch_path(&temp_dir, true);
            println!("Environment created - watching {:?}", temp_dir);
            Ok(env)
        }
    }));
    
    // Test initial state
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Initial")).unwrap();
    println!("Initial: {}", output);
    drop(env);
    
    // Now do a series of permission manipulations that might cause watcher errors
    println!("\nStarting permission manipulation stress test...");
    
    let reloader_clone = Arc::clone(&reloader);
    let temp_dir_clone = temp_dir.clone();
    let template_path_clone = template_path.clone();
    
    let permission_thread = thread::spawn(move || {
        for i in 0..20 {
            // Remove read permissions from directory
            let mut perms = fs::metadata(&temp_dir_clone).unwrap().permissions();
            perms.set_mode(0o000); // No permissions
            fs::set_permissions(&temp_dir_clone, perms).unwrap_or_else(|e| {
                println!("Failed to remove permissions (iteration {}): {}", i, e);
            });
            
            thread::sleep(Duration::from_millis(50));
            
            // Restore permissions
            let mut perms = match fs::metadata(&temp_dir_clone) {
                Ok(metadata) => metadata.permissions(),
                Err(_) => {
                    // If we can't read metadata, create default permissions
                    fs::Permissions::from_mode(0o755)
                }
            };
            perms.set_mode(0o755); // Full permissions
            fs::set_permissions(&temp_dir_clone, perms).unwrap_or_else(|e| {
                println!("Failed to restore permissions (iteration {}): {}", i, e);
            });
            
            // Make a file change while permissions are changing
            if i % 3 == 0 {
                let content = format!("Permission Version: {} - {{{{name}}}}", i + 2);
                fs::write(&template_path_clone, content).unwrap_or_else(|e| {
                    println!("Failed to write file (iteration {}): {}", i, e);
                });
            }
            
            thread::sleep(Duration::from_millis(50));
        }
    });
    
    // While permissions are being manipulated, try to use the autoreloader
    let usage_thread = thread::spawn(move || {
        for i in 0..30 {
            match reloader_clone.acquire_env() {
                Ok(env) => {
                    if let Ok(tmpl) = env.get_template("test.html") {
                        if let Ok(output) = tmpl.render(context!(name => format!("Usage-{}", i))) {
                            if i % 5 == 0 {
                                println!("Usage test {}: {}", i, output);
                            }
                        } else {
                            println!("Render failed at iteration {}", i);
                        }
                    } else {
                        println!("Template load failed at iteration {}", i);
                    }
                },
                Err(e) => {
                    println!("Environment acquisition failed at iteration {}: {}", i, e);
                }
            }
            
            thread::sleep(Duration::from_millis(100));
        }
    });
    
    permission_thread.join().unwrap();
    usage_thread.join().unwrap();
    
    println!("\n--- Final test after permission manipulation ---");
    
    // Ensure permissions are restored
    let mut perms = fs::metadata(&temp_dir).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&temp_dir, perms).unwrap();
    
    // Final change and test
    fs::write(&template_path, "Final Permission Test: FINAL - {{name}}").unwrap();
    thread::sleep(Duration::from_millis(500));
    
    let env = reloader.acquire_env().unwrap();
    let tmpl = env.get_template("test.html").unwrap();
    let output = tmpl.render(context!(name => "Final")).unwrap();
    println!("Final result: {}", output);
    
    if output.contains("FINAL") {
        println!("✅ Autoreload still works after permission manipulation");
    } else {
        println!("❌ Autoreload failed after permission manipulation");
    }
    
    // Clean up
    fs::remove_dir_all(&temp_dir).unwrap();
}