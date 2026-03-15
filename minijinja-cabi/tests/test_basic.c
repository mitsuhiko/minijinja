#include "testsupport.h"

#include <minijinja.h>

int main(void)
{
    mj_env *env = mj_env_new();
    TS_ASSERT(env != NULL);
    if (!env) {
        return ts_finish();
    }

    TS_ASSERT(mj_env_add_template(env, "hello", "Hello {{ name }}!"));

    mj_value ctx = mj_value_new_object();
    TS_ASSERT(mj_value_set_string_key(&ctx, "name", mj_value_new_string("C")));

    char *rendered = mj_env_render_template(env, "hello", ctx);
    TS_ASSERT_MSG(rendered != NULL, "rendering failed");
    if (rendered) {
        TS_ASSERT_STR_EQ(rendered, "Hello C!");
        mj_str_free(rendered);
    }

    mj_value expr_ctx = mj_value_new_object();
    TS_ASSERT(mj_value_set_string_key(&expr_ctx, "value", mj_value_new_i64(41)));

    mj_value rv = mj_env_eval_expr(env, "value + 1", expr_ctx);
    TS_ASSERT(!mj_err_is_set());
    TS_ASSERT_I64_EQ(mj_value_as_i64(rv), 42);
    mj_value_decref(&rv);

    mj_env_free(env);
    return ts_finish();
}
