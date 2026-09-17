# reth-contract-snapshot

Dumps the complete state of selected contracts — nonce, balance, code, and full storage — from a
live Reth datadir at an exact block, then replays `eth_call` simulations against that state in the
browser.

The datadir is opened read-only and cooperatively, so it runs alongside an active node. No JSON-RPC,
no second node, no network traffic. Calls execute in REVM compiled to WASM.

Typical use: inspect, debug, or fuzz a deployed contract (proxies, bridges, privacy pools) at a
historical block without an archive provider.

## Layout

| Crate | Contents |
| --- | --- |
| `snapshot-format` | Snapshot v1 schema, parsing, validation, trie roots. Reth-free; compiles for WASM. |
| `reth-contract-snapshot` | Native CLI. |
| `snapshot-web` | REVM executor, Alloy ABI handling, WASM API, and the TypeScript UI in `ui/`. |

Snapshot v1 is the only supported schema; there is no migration or compatibility mode.

## Producing a snapshot

Requires Rust 1.95 (`rust-toolchain.toml`) and a running Reth node on mainnet. The native crate is
pinned to Reth commit `68877d5fd7d146ef070bbe7dbabbe1097ea1da62`.

```bash
cargo run --release -p reth-contract-snapshot -- \
  --datadir /path/to/reth \
  --block 25891591 \
  --output snapshot.json \
  0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48,0xIMPLEMENTATION_ADDRESS
```

| Flag | Meaning |
| --- | --- |
| `--datadir <PATH>` | Reth datadir to open read-only. Required. |
| `<ADDRESSES>` | Comma-separated accounts, all pinned to the same block. Required. |
| `--block <N>` | Canonical block, decimal or `0x`-hex. Defaults to the latest available block. |
| `--output <FILE>` | Write to a file instead of stdout. |
| `--page-bytes <N>` | Response-byte limit per storage page (default 4 MiB). Tunes peak memory, not output size. |

Omit `--output` to stream JSON to stdout; progress and errors always go to stderr.

The output is one JSON file containing every requested account plus the block header, execution
context, and block-hash window they were taken from. Storage keys are Keccak-256 of raw 32-byte
slots. Nothing is written unless the export completes and its recomputed storage root matches the
account's — a failed run never leaves a partial or stale file behind.

### Proxy contracts

Dump proxy and implementation together at the same block:

```bash
cargo run --release -p reth-contract-snapshot -- \
  --datadir /path/to/reth --block 25891591 --output usdc.json \
  0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48,0xIMPLEMENTATION_ADDRESS
```

Sanity-check the result:

```bash
jq '.block' usdc.json
jq '.accounts[].account' usdc.json
jq '.accounts[] | select(.account.address == "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") | .storage | length' usdc.json
```

## Running calls in the browser

The **eth_call Sandbox** loads snapshot batches, attaches ABIs, and runs isolated `eth_call`
simulations. It makes no network requests at runtime.

```bash
cd crates/snapshot-web/ui
npm install
npm run dev -- --host 127.0.0.1     # dev server; builds the WASM module first
npm run build                        # static production build in dist/
```

Drop a snapshot into the state panel, attach an ABI to the target address (or paste raw calldata),
choose a function, fill in arguments, execute. Raw calldata and raw return data are always shown.

### Execution semantics

- Snapshot bytes go to a Web Worker and are parsed by Rust/WASM. Data is memory-only and never enters
  `localStorage`; validated ABI JSON is the only persisted state, keyed by lowercase address.
- Multiple batch files may be loaded only if execution context, block header, state root, and
  block-hash window all match. Duplicate accounts are rejected.
- Unsnapshotted accounts are execution errors, except an unsnapshotted origin acting as an empty EOA
  on a zero-value call; a non-zero `value` requires the caller's snapshot. If a call touches another
  ordinary account, snapshot it at the same block too.
- `BLOCKHASH` outside the captured window returns zero; a missing hash inside the window fails.
- Every call starts from the pristine snapshots and returned state changes are discarded, so calls are
  read-only and cannot affect one another.
- Alloy resolves ABI overloads by canonical signature, coerces typed arguments, encodes calldata, and
  decodes outputs and reverts (`Error(string)`, `Panic(uint256)`, custom errors).

### GitHub Pages

The `Deploy to GitHub Pages` workflow builds the WASM module and Vite app on every push to `master`
(or a manual run) and deploys `crates/snapshot-web/ui/dist/`. Published at
<https://3alpha.github.io/contract-snapshot-playground/>.

First-time setup, once per repository: **Settings → Pages** → set **Source** to **GitHub Actions**.
To redeploy without pushing, run the workflow from the **Actions** tab. `base: "./"` in
`vite.config.ts` keeps assets relative so the site works under the project path.

## Examples

`examples/` holds ready-to-load batch files for real protocols, each bundling several related
contracts at one shared block.