use minijinja::{context, Environment};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsValue, UnwrapThrowExt};

///
/// Will be the entry point for our WASM module.
///
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    let mut env = Environment::new();
    env.add_template("example", "Hello {{ name }}!")
        .unwrap_throw();
    // [unwrap_throw](https://rustwasm.github.io/wasm-bindgen/api/wasm_bindgen/trait.UnwrapThrowExt.html)
    // is a special function that will unwrap the value or throw a JS exception.
    let tmpl = env.get_template("example").unwrap_throw();
    let rendered = tmpl.render(context!(name => "WASM")).unwrap_throw();

    let window = web_sys::window().expect("should have a window in this context");
    let document = window.document().expect("window should have a document");

    // The output element is set in index.html
    let output_element = document
        .get_element_by_id("minijinja-output")
        .expect("should have #variables on the page");

    // We are setting the text content of the element to the rendered template.
    output_element.set_inner_html(&rendered);

    Ok(())
}
