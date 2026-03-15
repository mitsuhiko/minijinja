#include "testsupport.h"

#include <minijinja.h>

static int userdata_freed = 0;

static void userdata_free(void *userdata)
{
    free(userdata);
    userdata_freed++;
}

static bool multiply_function(void *userdata,
                              const mj_value *args,
                              uintptr_t argc,
                              mj_value *rv_out)
{
    int factor = userdata ? *((int *)userdata) : 1;
    int64_t value = argc > 0 ? mj_value_as_i64(args[0]) : 0;
    *rv_out = mj_value_new_i64(value * factor);
    return true;
}

static bool suffix_filter(void *userdata,
                          const mj_value *args,
                          uintptr_t argc,
                          mj_value *rv_out)
{
    const char *suffix = userdata ? (const char *)userdata : "";
    if (argc == 0) {
        *rv_out = mj_value_new_string(suffix);
        return true;
    }

    char *base = mj_value_to_str(args[0]);
    if (!base) {
        return false;
    }

    size_t base_len = strlen(base);
    size_t suffix_len = strlen(suffix);
    char *combined = malloc(base_len + suffix_len + 1);
    if (!combined) {
        mj_str_free(base);
        return false;
    }

    memcpy(combined, base, base_len);
    memcpy(combined + base_len, suffix, suffix_len + 1);

    *rv_out = mj_value_new_string(combined);

    free(combined);
    mj_str_free(base);
    return true;
}

static bool odd_test(void *userdata, const mj_value *args, uintptr_t argc, mj_value *rv_out)
{
    (void)userdata;
    int64_t value = argc > 0 ? mj_value_as_i64(args[0]) : 0;
    *rv_out = mj_value_new_bool((value % 2) != 0);
    return true;
}

int main(void)
{
    mj_env *env = mj_env_new();
    TS_ASSERT(env != NULL);
    if (!env) {
        return ts_finish();
    }

    int *factor = malloc(sizeof(int));
    TS_ASSERT(factor != NULL);
    if (!factor) {
        mj_env_free(env);
        return ts_finish();
    }
    *factor = 2;

    char *suffix = malloc(2);
    TS_ASSERT(suffix != NULL);
    if (!suffix) {
        free(factor);
        mj_env_free(env);
        return ts_finish();
    }
    suffix[0] = '!';
    suffix[1] = '\0';

    int *test_userdata = malloc(sizeof(int));
    TS_ASSERT(test_userdata != NULL);
    if (!test_userdata) {
        free(factor);
        free(suffix);
        mj_env_free(env);
        return ts_finish();
    }
    *test_userdata = 1;

    TS_ASSERT(mj_env_add_function(env, "mul", multiply_function, factor, userdata_free));
    TS_ASSERT(mj_env_add_filter(env, "suffix", suffix_filter, suffix, userdata_free));
    TS_ASSERT(mj_env_add_test(env, "odd", odd_test, test_userdata, userdata_free));

    TS_ASSERT(mj_env_add_global(env, "g", mj_value_new_string("G")));

    char *rendered = mj_env_render_named_str(env,
                                             "callbacks.txt",
                                             "{{ mul(21) }}|{{ 'hi'|suffix }}|{{ 3 is odd }}|{{ g }}",
                                             mj_value_new_object());

    TS_ASSERT_MSG(rendered != NULL, "callback render failed");
    if (rendered) {
        TS_ASSERT_STR_EQ(rendered, "42|hi!|true|G");
        mj_str_free(rendered);
    }

    mj_env_free(env);

    TS_ASSERT_I64_EQ(userdata_freed, 3);
    return ts_finish();
}
