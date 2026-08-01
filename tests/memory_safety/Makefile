# Makefile for the MCM Runtime Memory Safety Test Harness
#
# Usage:
#   make build    — Compile harness.c against libmcm_runtime.so
#   make run      — Run the harness (without Valgrind)
#   make valgrind — Run under Valgrind with full leak checking
#   make clean    — Remove build artifacts

CC       = gcc
CFLAGS   = -Wall -Wextra -std=gnu11 -g -O0
INCLUDES = -I../../include
LIB_DIR  = ../../backend_rust/target/debug
LDFLAGS  = -L$(LIB_DIR) -lmcm_runtime -Wl,-rpath,$(LIB_DIR)

TARGET   = harness

.PHONY: build run valgrind clean

build: $(TARGET)

$(TARGET): harness.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $< $(LDFLAGS)

run: build
	LD_LIBRARY_PATH=$(LIB_DIR) ./$(TARGET)

valgrind: build
	LD_LIBRARY_PATH=$(LIB_DIR) valgrind \
		--leak-check=full \
		--show-leak-kinds=all \
		--track-origins=yes \
		--error-exitcode=1 \
		--suppressions=valgrind.supp \
		./$(TARGET)

clean:
	rm -f $(TARGET)
