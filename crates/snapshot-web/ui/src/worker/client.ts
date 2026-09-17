import type { ApiResponse, WorkerAction, WorkerRequest, WorkerResponse } from "../types";

export class ExecutorClient {
  private readonly worker = new Worker(
    new URL("./executor.worker.ts", import.meta.url),
    { type: "module" },
  );
  private nextId = 1;
  private pending = new Map<number, (response: string) => void>();
  private unavailable: ApiResponse<never> | null = null;

  constructor() {
    this.worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
      const resolve = this.pending.get(event.data.id);
      if (resolve) {
        this.pending.delete(event.data.id);
        resolve(event.data.response);
      }
    };
    this.worker.onerror = (event) => {
      event.preventDefault();
      this.markUnavailable(
        "worker-failure",
        event.message || "The local execution worker stopped unexpectedly.",
      );
    };
    this.worker.onmessageerror = () => {
      this.markUnavailable(
        "worker-protocol",
        "The local execution worker returned an unreadable message.",
      );
    };
  }

  request<T>(
    action: WorkerAction,
    payload: Record<string, unknown> = {},
    transfer: Transferable[] = [],
  ): Promise<ApiResponse<T>> {
    if (this.unavailable) {
      return Promise.resolve(this.unavailable);
    }
    const id = this.nextId++;
    const message: WorkerRequest = { id, action, payload };
    return new Promise((resolve) => {
      this.pending.set(id, (response) => {
        try {
          resolve(JSON.parse(response) as ApiResponse<T>);
        } catch {
          resolve(
            failureResponse(
              "worker-protocol",
              "The local execution worker returned invalid JSON.",
            ),
          );
        }
      });
      try {
        this.worker.postMessage(message, transfer);
      } catch (error) {
        this.pending.delete(id);
        const detail = error instanceof Error ? error.message : String(error);
        resolve(
          failureResponse(
            "worker-send",
            "Failed to send a request to the local execution worker.",
            detail,
          ),
        );
      }
    });
  }

  private markUnavailable(code: string, message: string): void {
    this.unavailable = failureResponse(code, message);
    const response = JSON.stringify(this.unavailable);
    const pending = Array.from(this.pending.values());
    this.pending.clear();
    for (const resolve of pending) {
      resolve(response);
    }
  }
}

function failureResponse<T>(code: string, message: string, detail?: string): ApiResponse<T> {
  return {
    ok: false,
    data: null,
    error: {
      code,
      message,
      causes: detail ? [detail] : [],
    },
  };
}
