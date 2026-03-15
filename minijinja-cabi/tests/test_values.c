#include "testsupport.h"

#include <minijinja.h>

int main(void)
{
    mj_value v_none = mj_value_new_none();
    TS_ASSERT(mj_value_get_kind(v_none) == MJ_VALUE_KIND_NONE);
    TS_ASSERT(!mj_value_is_true(v_none));

    mj_value v_undef = mj_value_new_undefined();
    TS_ASSERT(mj_value_get_kind(v_undef) == MJ_VALUE_KIND_UNDEFINED);
    TS_ASSERT(!mj_value_is_true(v_undef));

    mj_value v_i32 = mj_value_new_i32(-5);
    TS_ASSERT_I64_EQ(mj_value_as_i64(v_i32), -5);

    mj_value v_u32 = mj_value_new_u32(7);
    TS_ASSERT_I64_EQ(mj_value_as_i64(v_u32), 7);

    mj_value v_u64 = mj_value_new_u64(42);
    TS_ASSERT(mj_value_as_u64(v_u64) == 42);
    TS_ASSERT(mj_value_is_number(v_u64));

    mj_value v_f32 = mj_value_new_f32(3.5f);
    TS_ASSERT_MSG(mj_value_as_f64(v_f32) > 3.49 && mj_value_as_f64(v_f32) < 3.51,
                  "f32 conversion should round-trip as f64");

    mj_value v_f64 = mj_value_new_f64(9.25);
    TS_ASSERT_MSG(mj_value_as_f64(v_f64) > 9.24 && mj_value_as_f64(v_f64) < 9.26,
                  "f64 conversion should round-trip");

    const char raw_bytes[] = {'a', '\0', 'b'};
    mj_value v_bytes = mj_value_new_bytes(raw_bytes, sizeof(raw_bytes));
    uintptr_t len = 0;
    const char *bytes_ptr = mj_value_as_bytes(v_bytes, &len);
    TS_ASSERT(bytes_ptr != NULL);
    TS_ASSERT(len == sizeof(raw_bytes));
    TS_ASSERT(((const unsigned char *)bytes_ptr)[0] == 'a');
    TS_ASSERT(((const unsigned char *)bytes_ptr)[1] == 0);
    TS_ASSERT(((const unsigned char *)bytes_ptr)[2] == 'b');

    mj_value v_str = mj_value_new_string("hello");
    const char *str_ptr = mj_value_as_bytes(v_str, &len);
    TS_ASSERT(str_ptr != NULL);
    TS_ASSERT(len == 5);
    TS_ASSERT(memcmp(str_ptr, "hello", 5) == 0);

    mj_value obj = mj_value_new_object();
    TS_ASSERT(mj_value_set_key(&obj, mj_value_new_string("answer"), mj_value_new_i64(42)));

    mj_value by_str = mj_value_get_by_str(obj, "answer");
    TS_ASSERT_I64_EQ(mj_value_as_i64(by_str), 42);

    mj_value by_value = mj_value_get_by_value(obj, mj_value_new_string("answer"));
    TS_ASSERT_I64_EQ(mj_value_as_i64(by_value), 42);

    mj_value list = mj_value_new_list();
    TS_ASSERT(mj_value_append(&list, mj_value_new_i64(1)));
    TS_ASSERT(mj_value_append(&list, mj_value_new_i64(2)));
    TS_ASSERT(mj_value_len(list) == 2);

    mj_value second = mj_value_get_by_index(list, 1);
    TS_ASSERT_I64_EQ(mj_value_as_i64(second), 2);

    mj_value_iter *iter = mj_value_try_iter(list);
    TS_ASSERT(iter != NULL);
    int64_t total = 0;
    int count = 0;
    if (iter) {
        mj_value item;
        while (mj_value_iter_next(iter, &item)) {
            total += mj_value_as_i64(item);
            count++;
            mj_value_decref(&item);
        }
        mj_value_iter_free(iter);
    }
    TS_ASSERT_I64_EQ(total, 3);
    TS_ASSERT(count == 2);

    mj_value dbg_value = mj_value_new_string("dbg");
    int saved_stderr = -1;
    if (ts_silence_stderr_begin(&saved_stderr)) {
        mj_value_dbg(dbg_value);
        ts_silence_stderr_end(saved_stderr);
    } else {
        TS_ASSERT_MSG(false, "failed to silence stderr for mj_value_dbg");
    }

    mj_value rc = mj_value_new_string("refcount");
    mj_value_incref(&rc);
    mj_value_decref(&rc);
    mj_value_decref(&rc);

    mj_value_decref(&dbg_value);
    mj_value_decref(&second);
    mj_value_decref(&list);
    mj_value_decref(&by_value);
    mj_value_decref(&by_str);
    mj_value_decref(&obj);
    mj_value_decref(&v_str);
    mj_value_decref(&v_bytes);
    mj_value_decref(&v_f64);
    mj_value_decref(&v_f32);
    mj_value_decref(&v_u64);
    mj_value_decref(&v_u32);
    mj_value_decref(&v_i32);
    mj_value_decref(&v_undef);
    mj_value_decref(&v_none);

    return ts_finish();
}
