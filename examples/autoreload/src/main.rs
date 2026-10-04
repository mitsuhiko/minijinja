use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use minijinja::context;
use minijinja::{path_loader, Environment};

fn main() {
    let template_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");

    let mut env = Environment::new();
    env.set_loader(path_loader(template_path));

    // Auto reloading is enabled by default: templates loaded via `path_loader`
    // are checked for changes every time they are looked up (including via
    // `extends`, `include` and `import`) and are reloaded individually.  If
    // DISABLE_AUTORELOAD is set, templates are only loaded once.
    if env::var("DISABLE_AUTORELOAD").as_deref() == Ok("1") {
        env.set_auto_reload(false);
    }

    // Reloading works through a shared reference, so the environment can be
    // placed in an `Arc` or a static without a lock.
    let env = Arc::new(env);

    // keep running the template.  to experiment change the template.txt file or
    // rename or change the include file.
    for iteration in 1..=u64::MAX {
        let tmpl = env.get_template("template.txt").unwrap();
        println!("{}", tmpl.render(context!(iteration)).unwrap());
        thread::sleep(Duration::from_secs(1));
    }
}
