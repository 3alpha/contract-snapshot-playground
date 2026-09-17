import type { WorkerRequest } from "../types";

export interface ExecutorBinding {
  addBatch(bytes: Uint8Array): string;
  removeSnapshot(address: string): string;
  accounts(): string;
  setAbi(address: string, json: string): string;
  removeAbi(address: string): string;
  execute(request: string): string;
}

export function dispatch(executor: ExecutorBinding, request: WorkerRequest): string {
  switch (request.action) {
    case "addBatch":
      return executor.addBatch(new Uint8Array(request.payload.bytes as ArrayBuffer));
    case "removeSnapshot":
      return executor.removeSnapshot(request.payload.address as string);
    case "accounts":
      return executor.accounts();
    case "setAbi":
      return executor.setAbi(
        request.payload.address as string,
        request.payload.json as string,
      );
    case "removeAbi":
      return executor.removeAbi(request.payload.address as string);
    case "execute":
      return executor.execute(JSON.stringify(request.payload));
  }
}
