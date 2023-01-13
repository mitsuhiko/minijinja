use std::collections::BTreeMap;

use minijinja::Environment;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

type Context = BTreeMap<String, serde_json::Value>;

#[function_component]
fn App() -> Html {
    let template = use_state(|| "Hello {{ name }}!\n\n{{ range(n) }}".to_string());
    let context = use_state(|| "{\n  \"name\": \"WebAssembly\",\n  \"n\": 5\n}\n".to_string());

    let on_template_input = {
        let template = template.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            template.set(input.value());
        })
    };

    let on_context_input = {
        let context = context.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            context.set(input.value());
        })
    };

    let mut env = Environment::new();
    env.set_debug(true);

    let rendered = match serde_json::from_str::<Context>(context.as_str()) {
        Ok(ctx) => match env.render_str(template.as_str(), ctx) {
            Ok(result) => result,
            Err(err) => format!("{:#}", err),
        },
        Err(err) => format!("JSON context error: {}", err),
    };

    html! {
        <div class="editor">
            <textarea oninput={on_template_input} value={template.to_string()}></textarea>
            <textarea oninput={on_context_input} value={context.to_string()}></textarea>
            <pre>{rendered}</pre>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
