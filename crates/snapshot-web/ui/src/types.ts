export interface ApiFailure {
  code: string;
  message: string;
  causes: string[];
}

export type ApiResponse<T> =
  | { ok: true; data: T; error: null }
  | { ok: false; data: null; error: ApiFailure };

export interface LoadedAccountSummary {
  address: string;
  nonce: string;
  balance: string;
  codeSize: number;
  storageEntries: number;
  blockNumber: string;
  blockHash: string;
}

export interface LoadedAccount extends LoadedAccountSummary {
  fileName?: string;
}

export interface AbiParameter {
  name: string;
  kind: string;
}

export interface AbiFunction {
  signature: string;
  name: string;
  stateMutability: string;
  inputs: AbiParameter[];
  outputs: AbiParameter[];
}

export interface AbiDescription {
  functions: AbiFunction[];
}

export interface CallResult {
  status: "success" | "revert" | "halt";
  gasUsed: number;
  calldata: string;
  output: string;
  decoded: string[] | null;
  reason: string | null;
}

export type WorkerAction =
  | "addBatch"
  | "removeSnapshot"
  | "accounts"
  | "setAbi"
  | "removeAbi"
  | "execute";

export interface WorkerRequest {
  id: number;
  action: WorkerAction;
  payload: Record<string, unknown>;
}

export interface WorkerResponse {
  id: number;
  response: string;
}
