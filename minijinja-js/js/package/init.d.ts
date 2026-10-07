import type { InitInput, SyncInitInput } from "./wasm/minijinja_js.js";

export * from "./types.js";
export type { InitInput, SyncInitInput };

/**
 * Initializes the wasm module asynchronously.
 *
 * If no argument is given, the wasm file is loaded relative to this module.
 */
export declare function init(
  moduleOrPath?: InitInput | Promise<InitInput>
): Promise<void>;

/**
 * Initializes the wasm module synchronously from bytes or a compiled module.
 */
export declare function initSync(module: SyncInitInput): void;

export default init;
