import type { RenderRequest, RenderResult } from "./protocol";

/** How long a render may take before the worker is terminated. */
const TIMEOUT_MS = 3000;

type Listener = (result: RenderResult) => void;

/**
 * Renders templates in a worker.
 *
 * Only one request is in flight at a time.  While a request is processed,
 * newer requests replace each other so that only the latest is rendered.
 * If a render does not finish in time, the worker is restarted.
 */
export class Renderer {
  private worker!: Worker;
  private ready!: Promise<void>;
  private inflight: RenderRequest | null = null;
  private pending: RenderRequest | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private nextId = 1;

  constructor(private listener: Listener) {
    this.start();
  }

  private start() {
    this.worker = new Worker(new URL("./worker.ts", import.meta.url), {
      type: "module",
    });
    this.ready = new Promise((resolve) => {
      this.worker.onmessage = (event) => {
        if ("ready" in event.data) {
          resolve();
        } else {
          this.onResult(event.data as RenderResult);
        }
      };
    });
  }

  render(request: Omit<RenderRequest, "id">) {
    this.pending = { ...request, id: this.nextId++ };
    if (!this.inflight) {
      void this.flush();
    }
  }

  private async flush() {
    const request = this.pending;
    if (!request) {
      return;
    }
    this.pending = null;
    this.inflight = request;
    await this.ready;
    this.worker.postMessage(request);
    this.timer = setTimeout(() => this.onTimeout(request), TIMEOUT_MS);
  }

  private onResult(result: RenderResult) {
    clearTimeout(this.timer);
    this.inflight = null;
    if (!this.pending) {
      this.listener(result);
    }
    void this.flush();
  }

  private onTimeout(request: RenderRequest) {
    this.worker.terminate();
    this.start();
    this.inflight = null;
    this.listener({
      id: request.id,
      diagnostics: [],
      error: {
        message: `Rendering did not finish within ${TIMEOUT_MS / 1000} seconds and was aborted.  Does the template contain an endless loop?`,
      },
    });
    void this.flush();
  }

  dispose() {
    clearTimeout(this.timer);
    this.worker.terminate();
  }
}
