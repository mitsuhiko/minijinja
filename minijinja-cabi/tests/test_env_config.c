#include "testsupport.h"

#include <minijinja.h>

int main(void)
{
    mj_env *env = mj_env_new();
    TS_ASSERT(env != NULL);
    if (!env) {
        return ts_finish();
    }

    mj_env_set_debug(env, true);
    mj_env_set_debug(env, false);
    mj_env_set_lstrip_blocks(env, true);
    mj_env_set_trim_blocks(env, true);
    mj_env_set_keep_trailing_newline(env, true);
    mj_env_set_recursion_limit(env, 64);

    mj_env_set_undefined_behavior(env, MJ_UNDEFINED_BEHAVIOR_LENIENT);
    mj_env_set_undefined_behavior(env, MJ_UNDEFINED_BEHAVIOR_CHAINABLE);
    mj_env_set_undefined_behavior(env, MJ_UNDEFINED_BEHAVIOR_STRICT);

    mj_syntax_config syntax;
    mj_syntax_config_default(&syntax);
    syntax.variable_start = "[[";
    syntax.variable_end = "]]";
    TS_ASSERT(mj_env_set_syntax_config(env, &syntax));

    TS_ASSERT(mj_env_add_template(env, "custom_syntax", "[[ 2 + 2 ]]"));
    char *custom = mj_env_render_template(env, "custom_syntax", mj_value_new_object());
    TS_ASSERT(custom != NULL);
    if (custom) {
        TS_ASSERT_STR_EQ(custom, "4");
        mj_str_free(custom);
    }

    TS_ASSERT(mj_env_add_template(env, "remove_me", "x"));
    TS_ASSERT(mj_env_remove_template(env, "remove_me"));
    char *removed = mj_env_render_template(env, "remove_me", mj_value_new_object());
    TS_ASSERT(removed == NULL);
    TS_ASSERT(mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_TEMPLATE_NOT_FOUND);
    mj_err_clear();

    TS_ASSERT(mj_env_add_template(env, "a", "A"));
    TS_ASSERT(mj_env_add_template(env, "b", "B"));
    TS_ASSERT(mj_env_clear_templates(env));

    char *after_clear = mj_env_render_template(env, "a", mj_value_new_object());
    TS_ASSERT(after_clear == NULL);
    TS_ASSERT(mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_TEMPLATE_NOT_FOUND);
    mj_err_clear();

    TS_ASSERT(mj_env_add_template(env, "newline", "line\n"));
    char *newline = mj_env_render_template(env, "newline", mj_value_new_object());
    TS_ASSERT(newline != NULL);
    if (newline) {
        TS_ASSERT_STR_EQ(newline, "line\n");
        mj_str_free(newline);
    }

    mj_syntax_config_default(&syntax);
    TS_ASSERT(mj_env_set_syntax_config(env, &syntax));

    TS_ASSERT(mj_env_add_template(env, "strict_undef", "{{ missing }}"));
    char *undef = mj_env_render_template(env, "strict_undef", mj_value_new_object());
    TS_ASSERT(undef == NULL);
    TS_ASSERT(mj_err_is_set());
    TS_ASSERT(mj_err_get_kind() == MJ_ERR_KIND_UNDEFINED_ERROR);
    mj_err_clear();

    mj_env_free(env);
    return ts_finish();
}
