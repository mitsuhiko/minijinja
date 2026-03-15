TARGET_DIR=../../target/debug
BUILD_DIR=.build
TESTS=test_basic test_callbacks test_loader_fuel test_values test_env_config test_errors

CFLAGS?=-std=c11 -Wall -Wextra -Werror

ifeq ($(shell uname -s),Darwin)
RUN_ENV=DYLD_LIBRARY_PATH=$(TARGET_DIR):$${DYLD_LIBRARY_PATH}
else
RUN_ENV=LD_LIBRARY_PATH=$(TARGET_DIR):$${LD_LIBRARY_PATH}
endif

.PHONY: test clean

test: $(TESTS:%=$(BUILD_DIR)/%)
	@set -e; \
	for test_bin in $^; do \
		echo "==> $$test_bin"; \
		$(RUN_ENV) $$test_bin; \
	done

$(BUILD_DIR)/%: %.c testsupport.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) -I../include $< -L$(TARGET_DIR) -lminijinja_cabi -o $@

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

clean:
	rm -rf $(BUILD_DIR)
