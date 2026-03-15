#include <minijinja.h>
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static bool sum_callback(void *userdata, const mj_value *args, uintptr_t argc, mj_value *rv_out)
{
    (void)userdata;
    int64_t total = 0;
    for (uintptr_t i = 0; i < argc; i++) {
        total += mj_value_as_i64(args[i]);
    }
    *rv_out = mj_value_new_i64(total);
    return true;
}

static bool shout_filter(void *userdata, const mj_value *args, uintptr_t argc, mj_value *rv_out)
{
    (void)userdata;
    if (argc == 0) {
        *rv_out = mj_value_new_string("!");
        return true;
    }

    char *text = mj_value_to_str(args[0]);
    if (!text) {
        return false;
    }

    size_t len = strlen(text);
    char *buf = malloc(len + 2);
    if (!buf) {
        mj_str_free(text);
        return false;
    }
    memcpy(buf, text, len);
    buf[len] = '!';
    buf[len + 1] = '\0';

    *rv_out = mj_value_new_string(buf);

    free(buf);
    mj_str_free(text);
    return true;
}

static bool even_test(void *userdata, const mj_value *args, uintptr_t argc, mj_value *rv_out)
{
    (void)userdata;
    if (argc == 0) {
        *rv_out = mj_value_new_bool(false);
        return true;
    }
    *rv_out = mj_value_new_bool((mj_value_as_i64(args[0]) % 2) == 0);
    return true;
}

static const char *loader_callback(void *userdata, const char *name)
{
    (void)userdata;
    if (strcmp(name, "partials/base.txt") == 0) {
        return "[loader] {% include 'item.txt' %}";
    }
    if (strcmp(name, "partials/item.txt") == 0) {
        return "{{ site_name }} :: {{ who|shout }} :: even={{ 4 is even }} :: sum={{ sum(1, 2, 3) }}";
    }
    return NULL;
}

static const char *path_join_callback(void *userdata, const char *name, const char *parent)
{
    (void)userdata;
    static char joined[256];

    const char *slash = strrchr(parent, '/');
    if (!slash) {
        return name;
    }

    size_t prefix_len = (size_t)(slash - parent) + 1;
    size_t name_len = strlen(name);
    if (prefix_len + name_len + 1 >= sizeof(joined)) {
        return NULL;
    }

    memcpy(joined, parent, prefix_len);
    memcpy(joined + prefix_len, name, name_len + 1);
    return joined;
}

static mj_auto_escape auto_escape_callback(void *userdata, const char *name)
{
    (void)userdata;
    if (!name) {
        return MJ_AUTO_ESCAPE_NONE;
    }
    const char *ext = strrchr(name, '.');
    if (ext && strcmp(ext, ".html") == 0) {
        return MJ_AUTO_ESCAPE_HTML;
    }
    return MJ_AUTO_ESCAPE_NONE;
}

int main()
{
    mj_env *env = mj_env_new();
    mj_env_set_debug(env, true);

    bool ok;

    // High-priority CABI extension hooks.
    ok = mj_env_add_function(env, "sum", sum_callback, NULL, NULL);
    if (!ok) {
        mj_err_print();
        return 1;
    }

    ok = mj_env_add_filter(env, "shout", shout_filter, NULL, NULL);
    if (!ok) {
        mj_err_print();
        return 1;
    }

    ok = mj_env_add_test(env, "even", even_test, NULL, NULL);
    if (!ok) {
        mj_err_print();
        return 1;
    }

    ok = mj_env_add_global(env, "site_name", mj_value_new_string("MiniJinja-CABI"));
    if (!ok) {
        mj_err_print();
        return 1;
    }

    // High-priority loader/path-join hooks.
    ok = mj_env_set_loader(env, loader_callback, NULL, NULL);
    if (!ok) {
        mj_err_print();
        return 1;
    }

    ok = mj_env_set_path_join_callback(env, path_join_callback, NULL, NULL);
    if (!ok) {
        mj_err_print();
        return 1;
    }

    // High-priority auto-escape hook.
    ok = mj_env_set_auto_escape_callback(env, auto_escape_callback, NULL, NULL);
    if (!ok) {
        mj_err_print();
        return 1;
    }

    // A regular in-memory template still works too.
    ok = mj_env_add_template(env, "hello", "Hello {{ name|shout }} from {{ site_name }}; sum={{ sum(1, 2, 3) }}; even={{ 42 is even }}");
    if (!ok) {
        mj_err_print();
        return 1;
    }

    mj_value ctx = mj_value_new_object();
    mj_value_set_string_key(&ctx, "name", mj_value_new_string("C-Lang"));

    char *rv = mj_env_render_template(env, "hello", ctx);
    if (!rv) {
        mj_err_print();
        return 1;
    }
    printf("%s\n", rv);
    mj_str_free(rv);

    // Render from callback-based loader (+ path-join callback for include).
    mj_value loader_ctx = mj_value_new_object();
    mj_value_set_string_key(&loader_ctx, "who", mj_value_new_string("loader"));
    char *lv = mj_env_render_template(env, "partials/base.txt", loader_ctx);
    if (!lv) {
        mj_err_print();
        return 1;
    }
    printf("%s\n", lv);
    mj_str_free(lv);

    // Render named HTML string to exercise auto-escape callback.
    mj_value esc_ctx = mj_value_new_object();
    mj_value_set_string_key(&esc_ctx, "dangerous", mj_value_new_string("<b>unsafe</b>"));
    char *ev = mj_env_render_named_str(env, "example.html", "{{ dangerous }}", esc_ctx);
    if (!ev) {
        mj_err_print();
        return 1;
    }
    printf("escaped: %s\n", ev);
    mj_str_free(ev);

    // High-priority fuel control.
    mj_env_set_fuel(env, 1);
    char *fv = mj_env_render_named_str(env, "fuel.txt", "{{ range(0, 1000)|list }}", mj_value_new_object());
    if (!fv) {
        fprintf(stderr, "expected fuel error:\n");
        mj_err_print();
        mj_err_clear();
    } else {
        mj_str_free(fv);
    }

    mj_env_clear_fuel(env);
    char *fok = mj_env_render_named_str(env, "fuel.txt", "{{ 1 + 2 }}", mj_value_new_object());
    if (!fok) {
        mj_err_print();
        return 1;
    }
    printf("fuel cleared: %s\n", fok);
    mj_str_free(fok);

    // Existing API snippets.
    mj_value erv = mj_env_eval_expr(env, "1 + 2", mj_value_new_object());
    char *ervs = mj_value_to_str(erv);
    fprintf(stderr, "1 + 2 = %s\n", ervs);
    mj_str_free(ervs);
    mj_value_decref(&erv);

    mj_value bv = mj_value_new_string("Hello");
    uintptr_t bvlen;
    const char *bvstr = mj_value_as_bytes(bv, &bvlen);
    assert(bvlen == 5);
    assert(memcmp(bvstr, "Hello", 5) == 0);
    mj_value_decref(&bv);

    mj_env_free(env);
    return 0;
}
