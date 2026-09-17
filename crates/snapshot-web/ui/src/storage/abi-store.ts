const PREFIX = "snapshot-call-lab:abi:";

export interface SavedAbi {
  address: string;
  abi: string;
}

export class AbiStore {
  constructor(private readonly storage: Storage) {}

  get(address: string): string | null {
    return this.storage.getItem(this.key(address));
  }

  list(): SavedAbi[] {
    const entries: SavedAbi[] = [];
    for (let index = 0; index < this.storage.length; index += 1) {
      const key = this.storage.key(index);
      if (!key?.startsWith(PREFIX)) continue;
      const address = key.slice(PREFIX.length);
      const abi = this.storage.getItem(key);
      if (abi) entries.push({ address, abi });
    }
    return entries.sort((left, right) => left.address.localeCompare(right.address));
  }

  replace(address: string, abi: string): void {
    this.storage.setItem(this.key(address), abi);
  }

  remove(address: string): void {
    this.storage.removeItem(this.key(address));
  }

  private key(address: string): string {
    return `${PREFIX}${address.toLowerCase()}`;
  }
}
