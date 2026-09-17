import { describe, expect, it } from "vitest";
import { AbiStore } from "./abi-store";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length(): number { return this.values.size; }
  clear(): void { this.values.clear(); }
  getItem(key: string): string | null { return this.values.get(key) ?? null; }
  key(index: number): string | null { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string): void { this.values.delete(key); }
  setItem(key: string, value: string): void { this.values.set(key, value); }
}

describe("AbiStore", () => {
  it("normalizes addresses and supports replace/remove", () => {
    const storage = new MemoryStorage();
    const store = new AbiStore(storage);
    store.replace("0xAbC", "first");
    expect(store.get("0xabc")).toBe("first");
    store.replace("0xABC", "second");
    expect(store.get("0xabc")).toBe("second");
    store.remove("0xAbC");
    expect(store.get("0xabc")).toBeNull();
  });

  it("lists every saved entry sorted by address and ignores foreign keys", () => {
    const storage = new MemoryStorage();
    storage.setItem("unrelated", "noise");
    const store = new AbiStore(storage);
    store.replace("0xddd", "d");
    store.replace("0xaaa", "a");
    store.replace("0xccc", "c");
    expect(store.list().map((entry) => entry.address)).toEqual(["0xaaa", "0xccc", "0xddd"]);
    expect(store.list()).toEqual([
      { address: "0xaaa", abi: "a" },
      { address: "0xccc", abi: "c" },
      { address: "0xddd", abi: "d" },
    ]);
  });
});
