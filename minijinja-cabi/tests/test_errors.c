#include "testsupport.h"

#include <minijinja.h>

int main(void)
{
    mj_env *env = mj_env_new();
    TS_ASSERT(env != NULL);
    if (!env) {
        return ts_finish();
    }

    TS_ASSERT(!mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_UNKNOWN);
    TS_ASSERT(mj_err_get_detail() == NULL);
    TS_ASSERT(mj_err_get_debug_info() == NULL);
    TS_ASSERT(mj_err_get_template_name() == NULL);
    TS_ASSERT(mj_err_get_line() == 0);
    TS_ASSERT(!mj_err_print());

    TS_ASSERT(!mj_env_add_template(env, "bad_syntax", "{% if %}"));
    TS_ASSERT(mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_SYNTAX_ERROR);
    TS_ASSERT(mj_err_get_line() > 0);

    char *detail = mj_err_get_detail();
    char *debug_info = mj_err_get_debug_info();
    char *template_name = mj_err_get_template_name();

    TS_ASSERT(detail != NULL);
    TS_ASSERT(debug_info != NULL);
    TS_ASSERT(template_name != NULL);
    if (template_name) {
        TS_ASSERT_STR_EQ(template_name, "bad_syntax");
    }

    int saved_stderr = -1;
    TS_ASSERT(ts_silence_stderr_begin(&saved_stderr));
    if (saved_stderr >= 0) {
        TS_ASSERT(mj_err_print());
        ts_silence_stderr_end(saved_stderr);
    }

    if (detail) {
        mj_str_free(detail);
    }
    if (debug_info) {
        mj_str_free(debug_info);
    }
    if (template_name) {
        mj_str_free(template_name);
    }

    mj_err_clear();
    TS_ASSERT(!mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_UNKNOWN);

    char *missing = mj_env_render_template(env, "does_not_exist", mj_value_new_object());
    TS_ASSERT(missing == NULL);
    TS_ASSERT(mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_TEMPLATE_NOT_FOUND);

    char *missing_name = mj_err_get_template_name();
    if (missing_name) {
        TS_ASSERT_STR_EQ(missing_name, "does_not_exist");
        mj_str_free(missing_name);
    }

    TS_ASSERT(mj_env_add_template(env, "ok", "hello"));
    TS_ASSERT(!mj_err_is_set());

    char *ok = mj_env_render_template(env, "ok", mj_value_new_object());
    TS_ASSERT(ok != NULL);
    if (ok) {
        TS_ASSERT_STR_EQ(ok, "hello");
        mj_str_free(ok);
    }

    mj_env_free(env);
    return ts_finish();
}
