#ifndef TESTSUPPORT_H
#define TESTSUPPORT_H

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <fcntl.h>
#include <unistd.h>

static int ts_failures = 0;

static void ts_fail(const char *file, int line, const char *expr, const char *msg)
{
    if (msg) {
        fprintf(stderr, "%s:%d: assertion failed: %s (%s)\n", file, line, expr, msg);
    } else {
        fprintf(stderr, "%s:%d: assertion failed: %s\n", file, line, expr);
    }
    ts_failures++;
}

#define TS_ASSERT(expr)                                                                          \
    do {                                                                                         \
        if (!(expr)) {                                                                           \
            ts_fail(__FILE__, __LINE__, #expr, NULL);                                            \
        }                                                                                        \
    } while (0)

#define TS_ASSERT_MSG(expr, message)                                                             \
    do {                                                                                         \
        if (!(expr)) {                                                                           \
            ts_fail(__FILE__, __LINE__, #expr, (message));                                       \
        }                                                                                        \
    } while (0)

#define TS_ASSERT_I64_EQ(actual, expected)                                                       \
    do {                                                                                         \
        int64_t ts_actual_ = (actual);                                                           \
        int64_t ts_expected_ = (expected);                                                       \
        if (ts_actual_ != ts_expected_) {                                                        \
            char ts_msg_[128];                                                                   \
            snprintf(ts_msg_, sizeof(ts_msg_), "actual=%lld expected=%lld",                    \
                     (long long)ts_actual_, (long long)ts_expected_);                            \
            ts_fail(__FILE__, __LINE__, #actual " == " #expected, ts_msg_);                    \
        }                                                                                        \
    } while (0)

#define TS_ASSERT_STR_EQ(actual, expected)                                                       \
    do {                                                                                         \
        const char *ts_actual_ = (actual);                                                       \
        const char *ts_expected_ = (expected);                                                   \
        if ((ts_actual_ == NULL) != (ts_expected_ == NULL) ||                                    \
            (ts_actual_ && strcmp(ts_actual_, ts_expected_) != 0)) {                             \
            char ts_msg_[256];                                                                   \
            snprintf(ts_msg_, sizeof(ts_msg_), "actual=\"%s\" expected=\"%s\"",           \
                     ts_actual_ ? ts_actual_ : "(null)",                                        \
                     ts_expected_ ? ts_expected_ : "(null)");                                   \
            ts_fail(__FILE__, __LINE__, #actual " == " #expected, ts_msg_);                    \
        }                                                                                        \
    } while (0)

#if defined(__GNUC__) || defined(__clang__)
#define TS_UNUSED __attribute__((unused))
#else
#define TS_UNUSED
#endif

static bool TS_UNUSED ts_silence_stderr_begin(int *saved_fd)
{
    int current_fd = dup(STDERR_FILENO);
    if (current_fd < 0) {
        return false;
    }

    int devnull_fd = open("/dev/null", O_WRONLY);
    if (devnull_fd < 0) {
        close(current_fd);
        return false;
    }

    if (dup2(devnull_fd, STDERR_FILENO) < 0) {
        close(devnull_fd);
        close(current_fd);
        return false;
    }

    close(devnull_fd);
    *saved_fd = current_fd;
    return true;
}

static void TS_UNUSED ts_silence_stderr_end(int saved_fd)
{
    fflush(stderr);
    dup2(saved_fd, STDERR_FILENO);
    close(saved_fd);
}

static int ts_finish(void)
{
    if (ts_failures == 0) {
        fprintf(stderr, "ok\n");
        return 0;
    }
    fprintf(stderr, "failed: %d assertion(s)\n", ts_failures);
    return 1;
}

#endif
