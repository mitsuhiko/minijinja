#!/bin/bash
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR/..
wasmtime run --max-wasm-stack=1048576 --env INSTA_WORKSPACE_ROOT=/ --mapdir "/::$(pwd)" -- "$@"
