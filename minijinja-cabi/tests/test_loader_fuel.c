#include "testsupport.h"

#include <minijinja.h>

static int loader_calls = 0;
static int path_join_calls = 0;
static int auto_escape_calls = 0;

static const char *loader_callback(void *userdata, const char *name)
{
    (void)userdata;
    loader_calls++;

    if (strcmp(name, "pages/base.html") == 0) {
        return "{% include 'frag.html' %}";
    }
    if (strcmp(name, "pages/frag.html") == 0) {
        return "<b>{{ value }}</b>";
    }
    return NULL;
}

static const char *path_join_callback(void *userdata, const char *name, const char *parent)
{
    (void)userdata;
    path_join_calls++;

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
    auto_escape_calls++;

    if (name) {
        size_t len = strlen(name);
        if (len >= 5 && strcmp(name + len - 5, ".html") == 0) {
            return MJ_AUTO_ESCAPE_HTML;
        }
    }
    return MJ_AUTO_ESCAPE_NONE;
}

int main(void)
{
    mj_env *env = mj_env_new();
    TS_ASSERT(env != NULL);
    if (!env) {
        return ts_finish();
    }

    TS_ASSERT(mj_env_set_loader(env, loader_callback, NULL, NULL));
    TS_ASSERT(mj_env_set_path_join_callback(env, path_join_callback, NULL, NULL));
    TS_ASSERT(mj_env_set_auto_escape_callback(env, auto_escape_callback, NULL, NULL));

    mj_value ctx = mj_value_new_object();
    TS_ASSERT(mj_value_set_string_key(&ctx, "value", mj_value_new_string("<x>")));

    char *rendered = mj_env_render_template(env, "pages/base.html", ctx);
    TS_ASSERT_MSG(rendered != NULL, "loader render failed");
    if (rendered) {
        TS_ASSERT_STR_EQ(rendered, "<b>&lt;x&gt;</b>");
        mj_str_free(rendered);
    }

    TS_ASSERT(loader_calls >= 2);
    TS_ASSERT(path_join_calls >= 1);
    TS_ASSERT(auto_escape_calls >= 1);

    mj_env_set_fuel(env, 1);
    char *blocked = mj_env_render_named_str(env,
                                            "fuel.txt",
                                            "{{ range(0, 1000)|list }}",
                                            mj_value_new_object());
    TS_ASSERT(blocked == NULL);
    TS_ASSERT(mj_err_is_set());
    mj_err_clear();

    mj_env_clear_fuel(env);
    char *ok = mj_env_render_named_str(env, "fuel.txt", "{{ 1 + 2 }}", mj_value_new_object());
    TS_ASSERT(ok != NULL);
    if (ok) {
        TS_ASSERT_STR_EQ(ok, "3");
        mj_str_free(ok);
    }

    mj_env_free(env);
    return ts_finish();
}
