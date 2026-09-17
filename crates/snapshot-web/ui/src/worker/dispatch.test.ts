import { describe, expect, it, vi } from "vitest";
import { dispatch, type ExecutorBinding } from "./dispatch";

function binding(): ExecutorBinding {
  return {
    addBatch: vi.fn(() => "added"),
    removeSnapshot: vi.fn(() => "removed"),
    accounts: vi.fn(() => "accounts"),
    setAbi: vi.fn(() => "abi"),
    removeAbi: vi.fn(() => "abi-removed"),
    execute: vi.fn(() => "executed"),
  };
}

describe("worker dispatch", () => {
  it("passes snapshot bytes only to the WASM executor", () => {
    const executor = binding();
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    expect(dispatch(executor, { id: 1, action: "addBatch", payload: { bytes } })).toBe("added");
    expect(executor.addBatch).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]));
  });

  it("serializes call requests at the worker boundary", () => {
    const executor = binding();
    const payload = { target: "0x01", mode: "raw", calldata: "0x" };
    expect(dispatch(executor, { id: 2, action: "execute", payload })).toBe("executed");
    expect(executor.execute).toHaveBeenCalledWith(JSON.stringify(payload));
  });
});
