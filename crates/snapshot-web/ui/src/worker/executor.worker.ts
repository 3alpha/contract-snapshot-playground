/// <reference lib="webworker" />

import init, { SnapshotExecutor } from "../generated/snapshot_web/snapshot_web.js";
import type { WorkerRequest, WorkerResponse } from "../types";
import { dispatch } from "./dispatch";

const scope = self as DedicatedWorkerGlobalScope;
let executor: SnapshotExecutor | undefined;

const ready = init().then(() => {
  executor = new SnapshotExecutor();
});

scope.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;
  let response: string;
  try {
    await ready;
    if (!executor) {
      throw new Error("WASM executor initialization completed without an executor instance");
    }
    response = dispatch(executor, request);
  } catch (error) {
    response = workerFailure(error);
  }
  const message: WorkerResponse = { id: request.id, response };
  scope.postMessage(message);
};

function workerFailure(error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  return JSON.stringify({
    ok: false,
    data: null,
    error: {
      code: "worker-failure",
      message: "The local execution worker failed.",
      causes: detail ? [detail] : [],
    },
  });
}
