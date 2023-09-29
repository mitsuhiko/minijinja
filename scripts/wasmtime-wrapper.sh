#!/bin/bash
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd $SCRIPT_DIR/..
wasmtime run -Wmax-wasm-stack=1048576 --env INSTA_WORKSPACE_ROOT=/ --mapdir "/::$(pwd)" -- "$@"
