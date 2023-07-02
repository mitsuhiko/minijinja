use std::iter::FromIterator;

use minijinja::value::{Kwargs, Value};
use minijinja::{Environment, Error, ErrorKind, State};

fn main() {
    let mut env = Environment::new();
    env.add_function(
        "custom_loop",
        |state: &State, num: i64, kwargs: Kwargs| -> Result<String, Error> {
            let caller = kwargs.get::<Value>("caller")?;
            kwargs.assert_all_used()?;
            let mut rv = String::new();
            for it in 0..num {
                let kwargs = Kwargs::from_iter([("it", Value::from(it + 1))]);
                rv.push_str(
                    caller
                        .call(state, &[kwargs.into()])?
                        .as_str()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::InvalidOperation,
                                "caller did not return a string",
                            )
                        })?,
                );
            }
            Ok(rv)
        },
    );

    let tmpl = env.template_from_str(include_str!("demo.txt")).unwrap();
    println!("{}", tmpl.render(()).unwrap());
}
