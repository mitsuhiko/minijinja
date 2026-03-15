use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::Arc;

use minijinja::syntax::SyntaxConfig;
use minijinja::value::Rest;
use minijinja::{AutoEscape, Environment, Error, ErrorKind, UndefinedBehavior, Value};

use crate::mj_value;

/// Pointer to a MiniJinja environment.
pub struct mj_env(pub(crate) Environment<'static>);

/// Callback used for user data cleanup.
pub type mj_user_data_free = Option<unsafe extern "C" fn(userdata: *mut c_void)>;

/// Callback used for custom functions, filters and tests.
///
/// Returns `true` on success and writes the return value into `rv_out`.
/// Returns `false` on failure.
pub type mj_value_callback = Option<
    unsafe extern "C" fn(
        userdata: *mut c_void,
        args: *const mj_value,
        argc: usize,
        rv_out: *mut mj_value,
    ) -> bool,
>;

/// Callback used for loading template source by name.
///
/// Return `NULL` if the template was not found.
pub type mj_loader_callback =
    Option<unsafe extern "C" fn(userdata: *mut c_void, name: *const c_char) -> *const c_char>;

/// Callback used to join include paths.
///
/// Return `NULL` to indicate an error.
pub type mj_path_join_callback = Option<
    unsafe extern "C" fn(
        userdata: *mut c_void,
        name: *const c_char,
        parent: *const c_char,
    ) -> *const c_char,
>;

/// Callback used to select auto escaping for a template name.
pub type mj_auto_escape_callback =
    Option<unsafe extern "C" fn(userdata: *mut c_void, name: *const c_char) -> mj_auto_escape>;

/// Auto escaping modes for callback-based configuration.
#[repr(C)]
pub enum mj_auto_escape {
    MJ_AUTO_ESCAPE_NONE,
    MJ_AUTO_ESCAPE_HTML,
}

struct UserData {
    userdata: *mut c_void,
    free_func: mj_user_data_free,
}

unsafe impl Send for UserData {}
unsafe impl Sync for UserData {}

impl Drop for UserData {
    fn drop(&mut self) {
        if let Some(free_func) = self.free_func {
            unsafe { free_func(self.userdata) };
        }
    }
}

fn missing_callback(which: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("missing {which} callback"),
    )
}

fn invalid_callback_result(which: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("{which} callback returned invalid utf-8"),
    )
}

fn callback_failed(which: &str) -> Error {
    Error::new(
        ErrorKind::InvalidOperation,
        format!("{which} callback failed"),
    )
}

fn invoke_value_callback(
    callback: unsafe extern "C" fn(
        userdata: *mut c_void,
        args: *const mj_value,
        argc: usize,
        rv_out: *mut mj_value,
    ) -> bool,
    userdata: &UserData,
    args: &[Value],
    which: &str,
) -> Result<Value, Error> {
    let mut c_args = args
        .iter()
        .cloned()
        .map(mj_value::from)
        .collect::<Vec<mj_value>>();
    let mut rv = mj_value::from(Value::UNDEFINED);

    let ok = unsafe { callback(userdata.userdata, c_args.as_ptr(), c_args.len(), &mut rv) };

    for arg in &mut c_args {
        unsafe {
            crate::mj_value_decref(arg as *mut _);
        }
    }

    if ok {
        Ok(rv.into_value())
    } else {
        unsafe {
            crate::mj_value_decref(&mut rv);
        }
        Err(callback_failed(which))
    }
}

ffi_fn! {
    /// Allocates a new and empty MiniJinja environment.
    unsafe fn mj_env_new(_scope) -> *mut mj_env {
        Box::into_raw(Box::new(mj_env(Environment::new())))
    }
}

ffi_fn! {
    /// Frees a MiniJinja environment.
    unsafe fn mj_env_free(_scope, env: *mut mj_env) {
        if !env.is_null() {
            drop(Box::from_raw(env));
        }
    }
}

ffi_fn! {
    /// Registers a template with the environment.
    ///
    /// This returns `false` if the template is malformed.
    unsafe fn mj_env_add_template(
        scope,
        env: *mut mj_env,
        name: *const c_char,
        source: *const c_char
    ) -> bool {
        (*env).0.add_template_owned(
            scope.get_str(name)?.to_string(),
            scope.get_str(source)?.to_string()
        )?;
        true
    }
}

ffi_fn! {
    /// Removes a template from the environment.
    ///
    /// This returns `false` if the template is malformed.
    unsafe fn mj_env_remove_template(
        scope,
        env: *mut mj_env,
        name: *const c_char,
    ) -> bool {
        (*env).0.remove_template(scope.get_str(name)?);
        true
    }
}

ffi_fn! {
    /// Clears all templates.
    unsafe fn mj_env_clear_templates(
        _scope,
        env: *mut mj_env,
    ) -> bool {
        (*env).0.clear_templates();
        true
    }
}

ffi_fn! {
    /// Adds a global value to the environment.
    ///
    /// Takes ownership of the given value.
    unsafe fn mj_env_add_global(
        scope,
        env: *mut mj_env,
        name: *const c_char,
        value: mj_value,
    ) -> bool {
        let value = value.into_value();
        (*env).0.add_global(scope.get_str(name)?.to_string(), value);
        true
    }
}

ffi_fn! {
    /// Registers a C callback as template function.
    unsafe fn mj_env_add_function(
        scope,
        env: *mut mj_env,
        name: *const c_char,
        callback: mj_value_callback,
        userdata: *mut c_void,
        free_func: mj_user_data_free,
    ) -> bool {
        let callback = callback.ok_or_else(|| missing_callback("function"))?;
        let name = scope.get_str(name)?.to_string();
        let userdata = Arc::new(UserData { userdata, free_func });
        (*env).0.add_function(name, {
            let userdata = userdata.clone();
            move |args: Rest<Value>| -> Result<Value, Error> {
                invoke_value_callback(callback, userdata.as_ref(), &args, "function")
            }
        });
        true
    }
}

ffi_fn! {
    /// Registers a C callback as filter.
    unsafe fn mj_env_add_filter(
        scope,
        env: *mut mj_env,
        name: *const c_char,
        callback: mj_value_callback,
        userdata: *mut c_void,
        free_func: mj_user_data_free,
    ) -> bool {
        let callback = callback.ok_or_else(|| missing_callback("filter"))?;
        let name = scope.get_str(name)?.to_string();
        let userdata = Arc::new(UserData { userdata, free_func });
        (*env).0.add_filter(name, {
            let userdata = userdata.clone();
            move |args: Rest<Value>| -> Result<Value, Error> {
                invoke_value_callback(callback, userdata.as_ref(), &args, "filter")
            }
        });
        true
    }
}

ffi_fn! {
    /// Registers a C callback as test.
    unsafe fn mj_env_add_test(
        scope,
        env: *mut mj_env,
        name: *const c_char,
        callback: mj_value_callback,
        userdata: *mut c_void,
        free_func: mj_user_data_free,
    ) -> bool {
        let callback = callback.ok_or_else(|| missing_callback("test"))?;
        let name = scope.get_str(name)?.to_string();
        let userdata = Arc::new(UserData { userdata, free_func });
        (*env).0.add_test(name, {
            let userdata = userdata.clone();
            move |args: Rest<Value>| -> Result<Value, Error> {
                invoke_value_callback(callback, userdata.as_ref(), &args, "test")
            }
        });
        true
    }
}

ffi_fn! {
    /// Configures a callback-based template loader.
    unsafe fn mj_env_set_loader(
        _scope,
        env: *mut mj_env,
        callback: mj_loader_callback,
        userdata: *mut c_void,
        free_func: mj_user_data_free,
    ) -> bool {
        let callback = callback.ok_or_else(|| missing_callback("loader"))?;
        let userdata = Arc::new(UserData { userdata, free_func });
        (*env).0.set_loader({
            let userdata = userdata.clone();
            move |name| {
                let name = CString::new(name).map_err(|_| callback_failed("loader"))?;
                let rv = unsafe { callback(userdata.userdata, name.as_ptr()) };
                if rv.is_null() {
                    Ok(None)
                } else {
                    let source = unsafe { CStr::from_ptr(rv) }
                        .to_str()
                        .map_err(|_| invalid_callback_result("loader"))?;
                    Ok(Some(source.to_string()))
                }
            }
        });
        true
    }
}

ffi_fn! {
    /// Configures a callback for joining include paths.
    unsafe fn mj_env_set_path_join_callback(
        _scope,
        env: *mut mj_env,
        callback: mj_path_join_callback,
        userdata: *mut c_void,
        free_func: mj_user_data_free,
    ) -> bool {
        let callback = callback.ok_or_else(|| missing_callback("path join"))?;
        let userdata = Arc::new(UserData { userdata, free_func });
        (*env).0.set_path_join_callback({
            let userdata = userdata.clone();
            move |name, parent| -> std::borrow::Cow<'_, str> {
                let Ok(name_cstr) = CString::new(name) else {
                    return name.into();
                };
                let Ok(parent_cstr) = CString::new(parent) else {
                    return name.into();
                };

                let rv = unsafe {
                    callback(userdata.userdata, name_cstr.as_ptr(), parent_cstr.as_ptr())
                };

                if rv.is_null() {
                    name.into()
                } else {
                    unsafe { CStr::from_ptr(rv) }
                        .to_str()
                        .map_or_else(|_| name.into(), |joined| joined.to_string().into())
                }
            }
        });
        true
    }
}

ffi_fn! {
    /// Configures a callback for auto escaping.
    unsafe fn mj_env_set_auto_escape_callback(
        _scope,
        env: *mut mj_env,
        callback: mj_auto_escape_callback,
        userdata: *mut c_void,
        free_func: mj_user_data_free,
    ) -> bool {
        let callback = callback.ok_or_else(|| missing_callback("auto escape"))?;
        let userdata = Arc::new(UserData { userdata, free_func });
        (*env).0.set_auto_escape_callback({
            let userdata = userdata.clone();
            move |name| {
                let rv = if let Ok(name) = CString::new(name) {
                    unsafe { callback(userdata.userdata, name.as_ptr()) }
                } else {
                    unsafe { callback(userdata.userdata, ptr::null()) }
                };
                match rv {
                    mj_auto_escape::MJ_AUTO_ESCAPE_NONE => AutoEscape::None,
                    mj_auto_escape::MJ_AUTO_ESCAPE_HTML => AutoEscape::Html,
                }
            }
        });
        true
    }
}

ffi_fn! {
    /// Renders a template registered on the environment.
    ///
    /// Takes ownership of the given context.
    unsafe fn mj_env_render_template(
        scope,
        env: *const mj_env,
        name: *const c_char,
        ctx: mj_value
    ) -> *mut c_char {
        let ctx = ctx.into_value();
        let t = (*env).0.get_template(scope.get_str(name)?)?;
        let rv = t.render(ctx)?;
        CString::new(rv).map_err(|_| {
            Error::new(ErrorKind::InvalidOperation, "template rendered null bytes")
        })?.into_raw()
    }
}

ffi_fn! {
    /// Renders a template from a named string.
    ///
    /// Takes ownership of the given context.
    unsafe fn mj_env_render_named_str(
        scope,
        env: *const mj_env,
        name: *const c_char,
        source: *const c_char,
        ctx: mj_value
    ) -> *mut c_char {
        let ctx = ctx.into_value();
        let rv = (*env).0.render_named_str(
            scope.get_str(name)?,
            scope.get_str(source)?,
            ctx
        )?;
        CString::new(rv).map_err(|_| {
            Error::new(ErrorKind::InvalidOperation, "template rendered null bytes")
        })?.into_raw()
    }
}

ffi_fn! {
    /// Evaluate an expression.
    unsafe fn mj_env_eval_expr(
        scope,
        env: *const mj_env,
        expr: *const c_char,
        ctx: mj_value
    ) -> mj_value {
        let ctx = ctx.into_value();
        let expr = (*env).0.compile_expression(scope.get_str(expr)?)?;
        expr.eval(ctx)?.into()
    }
}

ffi_fn! {
    /// Frees an engine allocated string.
    unsafe fn mj_str_free(_scope, s: *mut c_char) {
        if !s.is_null() {
            let _ = CString::from_raw(s);
        }
    }
}

ffi_fn! {
    /// Enables or disables the `lstrip_blocks` feature.
    unsafe fn mj_env_set_lstrip_blocks(_scope, env: *mut mj_env, val: bool) {
        (*env).0.set_lstrip_blocks(val);
    }
}

ffi_fn! {
    /// Enables or disables the `trim_blocks` feature.
    unsafe fn mj_env_set_trim_blocks(_scope, env: *mut mj_env, val: bool) {
        (*env).0.set_trim_blocks(val);
    }
}

ffi_fn! {
    /// Preserve the trailing newline when rendering templates.
    unsafe fn mj_env_set_keep_trailing_newline(_scope, env: *mut mj_env, val: bool) {
        (*env).0.set_keep_trailing_newline(val);
    }
}

ffi_fn! {
    /// Sets the fuel budget for expression evaluation and rendering.
    unsafe fn mj_env_set_fuel(_scope, env: *mut mj_env, fuel: u64) {
        (*env).0.set_fuel(Some(fuel));
    }
}

ffi_fn! {
    /// Clears the fuel budget.
    unsafe fn mj_env_clear_fuel(_scope, env: *mut mj_env) {
        (*env).0.set_fuel(None);
    }
}

/// Allows one to override the syntax elements.
#[repr(C)]
pub struct mj_syntax_config {
    block_start: *const c_char,
    block_end: *const c_char,
    variable_start: *const c_char,
    variable_end: *const c_char,
    comment_start: *const c_char,
    comment_end: *const c_char,
    line_statement_prefix: *const c_char,
    line_comment_prefix: *const c_char,
}

const DEFAULT_BLOCK_START: &[u8] = b"{%\0";
const DEFAULT_BLOCK_END: &[u8] = b"%}\0";
const DEFAULT_VARIABLE_START: &[u8] = b"{{\0";
const DEFAULT_VARIABLE_END: &[u8] = b"}}\0";
const DEFAULT_COMMENT_START: &[u8] = b"{#\0";
const DEFAULT_COMMENT_END: &[u8] = b"#}\0";

#[inline]
const fn c_char_ptr(bytes: &'static [u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

ffi_fn! {
    /// Reconfigures the syntax.
    unsafe fn mj_env_set_syntax_config(scope, env: *mut mj_env, syntax: &mj_syntax_config) -> bool {
        let mut builder = SyntaxConfig::builder();
        builder
            .block_delimiters(
                scope.get_str(syntax.block_start)?.to_string(),
                scope.get_str(syntax.block_end)?.to_string()
            )
            .variable_delimiters(
                scope.get_str(syntax.variable_start)?.to_string(),
                scope.get_str(syntax.variable_end)?.to_string()
            )
            .comment_delimiters(
                scope.get_str(syntax.comment_start)?.to_string(),
                scope.get_str(syntax.comment_end)?.to_string()
            );
        let line_statement_prefix = scope.get_str(syntax.line_statement_prefix)?;
        if !line_statement_prefix.is_empty() {
            builder.line_statement_prefix(line_statement_prefix.to_string());
        }
        let line_comment_prefix = scope.get_str(syntax.line_comment_prefix)?;
        if !line_comment_prefix.is_empty() {
            builder.line_comment_prefix(line_comment_prefix.to_string());
        }
        (*env).0.set_syntax(builder.build()?);
        true
    }
}

ffi_fn! {
    /// Sets the syntax to defaults.
    unsafe fn mj_syntax_config_default(_scope, syntax: &mut mj_syntax_config) {
        syntax.block_start = c_char_ptr(DEFAULT_BLOCK_START);
        syntax.block_end = c_char_ptr(DEFAULT_BLOCK_END);
        syntax.variable_start = c_char_ptr(DEFAULT_VARIABLE_START);
        syntax.variable_end = c_char_ptr(DEFAULT_VARIABLE_END);
        syntax.comment_start = c_char_ptr(DEFAULT_COMMENT_START);
        syntax.comment_end = c_char_ptr(DEFAULT_COMMENT_END);
        syntax.line_statement_prefix = ptr::null();
        syntax.line_comment_prefix = ptr::null();
    }
}

/// Controls the undefined behavior of the engine.
#[repr(C)]
pub enum mj_undefined_behavior {
    /// The default, somewhat lenient undefined behavior.
    MJ_UNDEFINED_BEHAVIOR_LENIENT,
    /// Complains very quickly about undefined values.
    MJ_UNDEFINED_BEHAVIOR_STRICT,
    /// Like Lenient, but also allows chaining of undefined lookups.
    MJ_UNDEFINED_BEHAVIOR_CHAINABLE,
}

ffi_fn! {
    /// Reconfigures the undefined behavior.
    unsafe fn mj_env_set_undefined_behavior(_scope, env: *mut mj_env, val: mj_undefined_behavior) {
        (*env).0.set_undefined_behavior(match val {
            mj_undefined_behavior::MJ_UNDEFINED_BEHAVIOR_LENIENT => UndefinedBehavior::Lenient,
            mj_undefined_behavior::MJ_UNDEFINED_BEHAVIOR_STRICT => UndefinedBehavior::Strict,
            mj_undefined_behavior::MJ_UNDEFINED_BEHAVIOR_CHAINABLE => UndefinedBehavior::Chainable,
        })
    }
}

ffi_fn! {
    /// Enables or disables debug mode.
    unsafe fn mj_env_set_debug(_scope, env: *mut mj_env, val: bool) {
        (*env).0.set_debug(val);
    }
}

ffi_fn! {
    /// Changes the recursion limit.
    unsafe fn mj_env_set_recursion_limit(_scope, env: *mut mj_env, val: u32) {
        (*env).0.set_recursion_limit(val as _);
    }
}
