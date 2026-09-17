import "./styles.css";
import { AbiStore } from "./storage/abi-store";
import type {
  AbiDescription,
  AbiFunction,
  AbiParameter,
  ApiFailure,
  CallResult,
  LoadedAccount,
  LoadedAccountSummary,
} from "./types";
import { ExecutorClient } from "./worker/client";

const client = new ExecutorClient();
const abiStore = new AbiStore(localStorage);
const accounts = new Map<string, LoadedAccount>();
const abiDescriptions = new Map<string, AbiDescription>();
const NEW_ADDRESS_VALUE = "__new__";
let rawMode = false;
let executing = false;

function element<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (!found) {
    throw new Error(`missing UI element ${selector}`);
  }
  return found;
}

const snapshotInput = element<HTMLInputElement>("#snapshot-files");
const dropZone = element<HTMLElement>("#drop-zone");
const target = element<HTMLSelectElement>("#target");
const abiAddress = element<HTMLSelectElement>("#abi-address");
const abiAddressNewRow = element<HTMLElement>("#abi-address-new-row");
const abiAddressNew = element<HTMLInputElement>("#abi-address-new");
const abiSavedCount = element<HTMLElement>("#abi-saved-count");
const abiJson = element<HTMLTextAreaElement>("#abi-json");
const functionSelect = element<HTMLSelectElement>("#function");

snapshotInput.addEventListener("change", () => void loadFiles(snapshotInput.files));
dropZone.addEventListener("dragover", (event) => {
  event.preventDefault();
  dropZone.classList.add("dragging");
});
dropZone.addEventListener("dragleave", () => dropZone.classList.remove("dragging"));
dropZone.addEventListener("drop", (event) => {
  event.preventDefault();
  dropZone.classList.remove("dragging");
  void loadFiles(event.dataTransfer?.files ?? null);
});

element<HTMLInputElement>("#abi-file").addEventListener("change", async (event) => {
  const input = event.currentTarget as HTMLInputElement;
  const file = input.files?.[0];
  if (file) {
    abiJson.value = await file.text();
    syncAbiButtons();
  }
});
element<HTMLButtonElement>("#save-abi").addEventListener("click", () => void saveAbi());
element<HTMLButtonElement>("#remove-abi").addEventListener("click", () => void removeAbi());
abiAddress.addEventListener("change", () => void onAbiAddressChange());
abiAddressNew.addEventListener("input", syncAbiButtons);
abiJson.addEventListener("input", syncAbiButtons);
abiJson.addEventListener("paste", () => requestAnimationFrame(syncAbiButtons));
target.addEventListener("change", () => void selectTarget());
functionSelect.addEventListener("change", renderArguments);
element<HTMLButtonElement>("#abi-mode").addEventListener("click", () => setRawMode(false));
element<HTMLButtonElement>("#raw-mode").addEventListener("click", () => setRawMode(true));
element<HTMLButtonElement>("#execute").addEventListener("click", () => void execute());
refreshAbiAddressOptions();
syncAbiButtons();
syncExecution();
document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    if (!element<HTMLButtonElement>("#execute").disabled) void execute();
  }
});
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-copy]")) {
  button.addEventListener("click", async () => {
    const output = element<HTMLElement>(`#${button.dataset.copy}`).textContent ?? "";
    try {
      await navigator.clipboard.writeText(output);
      setStatus("#copy-status", "Copied to clipboard.", "success");
    } catch {
      setStatus("#copy-status", "Clipboard unavailable. Select and copy the output manually.", "error");
    }
  });
}

function syncExecution(): void {
  const ready = Boolean(target.value) && (rawMode || Boolean(functionSelect.value));
  element<HTMLButtonElement>("#execute").disabled = executing || !ready;
  target.disabled = accounts.size === 0;
  functionSelect.disabled = !functionSelect.value;
  if (!executing) {
    const status = element<HTMLElement>("#call-status");
    if (!ready) {
      status.className = "status";
      status.textContent = target.value
        ? "Save an ABI for this target, or switch to raw calldata."
        : "Import a snapshot to get started.";
    } else if (status.className === "status") {
      status.textContent = "Ready. Calls run against snapshot state without persisting changes.";
    }
  }
}

async function loadFiles(files: FileList | null): Promise<void> {
  if (!files?.length) return;
  setStatus("#snapshot-status", `Validating ${files.length} file(s)…`, "working");
  for (const file of Array.from(files)) {
    const bytes = await file.arrayBuffer();
    const response = await client.request<LoadedAccountSummary[]>(
      "addBatch",
      { bytes },
      [bytes],
    );
    if (!response.ok) {
      setStatus("#snapshot-status", `${file.name}: ${failureText(response.error)}`, "error");
      renderAccounts();
      rebuildTargets();
      return;
    }
    for (const summary of response.data) {
      accounts.set(summary.address, { ...summary, fileName: file.name });
    }
  }
  setStatus("#snapshot-status", "All files validated, root checked, and held in the worker.", "success");
  renderAccounts();
  rebuildTargets();
}

function renderAccounts(): void {
  const list = element<HTMLElement>("#accounts");
  list.replaceChildren();
  for (const account of accounts.values()) {
    const card = document.createElement("div");
    card.className = "account-card";
    const details = document.createElement("button");
    details.type = "button";
    details.className = "account-select";
    details.setAttribute("aria-label", `Use ${account.address} as the call target`);
    details.setAttribute("aria-pressed", String(account.address === target.value));
    card.classList.toggle("selected", account.address === target.value);
    const address = document.createElement("code");
    address.textContent = account.address;
    const meta = document.createElement("small");
    meta.textContent = `${account.storageEntries.toLocaleString()} slots · ${account.codeSize.toLocaleString()} code bytes · block ${account.blockNumber}`;
    details.append(address, meta);
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "icon-button";
    remove.title = `Remove ${account.address}`;
    remove.textContent = "×";
    remove.addEventListener("click", (event) => {
      event.stopPropagation();
      void removeSnapshot(account.address);
    });
    details.addEventListener("click", () => void selectContract(account.address));
    card.append(details, remove);
    list.append(card);
  }
  element<HTMLElement>("#accounts-empty").hidden = accounts.size > 0;
  element<HTMLElement>("#account-count").textContent = `${accounts.size} loaded`;
}

async function selectContract(address: string): Promise<void> {
  if (!accounts.has(address)) return;
  target.value = address;
  renderAccounts();
  await focusAbiAddress(address);
  target.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

async function removeSnapshot(address: string): Promise<void> {
  const response = await client.request<void>("removeSnapshot", { address });
  if (!response.ok) {
    setStatus("#snapshot-status", failureText(response.error), "error");
    return;
  }
  accounts.delete(address);
  renderAccounts();
  rebuildTargets();
}

function rebuildTargets(): void {
  const selected = target.value;
  target.replaceChildren();
  if (!accounts.size) {
    target.add(new Option("Load a snapshot first", ""));
  } else {
    for (const address of accounts.keys()) target.add(new Option(address, address));
    target.value = accounts.has(selected) ? selected : accounts.keys().next().value ?? "";
  }
  void selectTarget();
}

async function selectTarget(): Promise<void> {
  const targetAddress = target.value;
  renderAccounts();
  if (!targetAddress) {
    renderFunctions();
    return;
  }
  await focusAbiAddress(targetAddress);
}

function activeAbiAddress(): string {
  if (abiAddress.value === NEW_ADDRESS_VALUE) {
    return abiAddressNew.value.trim().toLowerCase();
  }
  return abiAddress.value.trim().toLowerCase();
}

async function focusAbiAddress(address: string): Promise<void> {
  const saved = abiStore.list().some((entry) => entry.address === address);
  if (saved) {
    setAbiAddressSelection(address);
    abiAddressNewRow.hidden = true;
    abiAddressNew.value = "";
  } else {
    setAbiAddressSelection(NEW_ADDRESS_VALUE);
    abiAddressNewRow.hidden = false;
    abiAddressNew.value = address;
  }
  await loadAbiForAddress(address);
}

function setAbiAddressSelection(value: string): void {
  const has = Array.from(abiAddress.options).some((option) => option.value === value);
  abiAddress.value = has ? value : "";
}

async function onAbiAddressChange(): Promise<void> {
  const value = abiAddress.value;
  if (value === NEW_ADDRESS_VALUE) {
    abiAddressNewRow.hidden = false;
    abiAddressNew.value = "";
    abiJson.value = "";
    abiAddressNew.focus();
    syncAbiButtons();
    return;
  }
  abiAddressNewRow.hidden = true;
  abiAddressNew.value = "";
  if (!value) {
    abiJson.value = "";
    abiDescriptions.delete(value);
    renderFunctions();
    return;
  }
  await loadAbiForAddress(value);
}

async function loadAbiForAddress(address: string): Promise<void> {
  const saved = abiStore.get(address) ?? "";
  abiJson.value = saved;
  if (!saved) {
    abiDescriptions.delete(address);
    renderFunctions();
    syncAbiButtons();
    return;
  }
  const response = await client.request<AbiDescription>("setAbi", { address, json: saved });
  if (response.ok) {
    abiDescriptions.set(address, response.data);
    setStatus("#abi-status", `${response.data.functions.length} callable function(s) loaded from saved ABI.`, "success");
  } else {
    abiDescriptions.delete(address);
    setStatus("#abi-status", `Saved ABI is no longer valid: ${failureText(response.error)}`, "error");
  }
  renderFunctions();
  syncAbiButtons();
}

function refreshAbiAddressOptions(preferred?: string): void {
  const saved = abiStore.list();
  const previouslySelected = preferred ?? abiAddress.value;
  abiAddress.replaceChildren();
  abiAddress.add(new Option("— Pick a saved contract —", ""));
  for (const entry of saved) {
    abiAddress.add(new Option(entry.address, entry.address));
  }
  abiAddress.add(new Option("+ Add new address…", NEW_ADDRESS_VALUE));
  if (previouslySelected && Array.from(abiAddress.options).some((option) => option.value === previouslySelected)) {
    abiAddress.value = previouslySelected;
  }
  abiSavedCount.textContent = saved.length === 1 ? "1 saved" : `${saved.length} saved`;
  syncAbiButtons();
}

function syncAbiButtons(): void {
  const address = activeAbiAddress();
  const hasAddress = address.length > 0;
  element<HTMLButtonElement>("#save-abi").disabled = !hasAddress || !abiJson.value.trim();
  element<HTMLButtonElement>("#remove-abi").disabled = !hasAddress || !abiStore.get(address);
}

async function saveAbi(): Promise<void> {
  const address = activeAbiAddress();
  const json = abiJson.value.trim();
  if (!address) {
    setStatus("#abi-status", "Choose or add a contract address first.", "error");
    return;
  }
  if (!json) {
    setStatus("#abi-status", "Paste or choose an ABI JSON before saving.", "error");
    return;
  }
  setStatus("#abi-status", "Validating ABI in the worker…", "working");
  const response = await client.request<AbiDescription>("setAbi", { address, json });
  if (!response.ok) {
    setStatus("#abi-status", failureText(response.error), "error");
    return;
  }
  abiStore.replace(address, json);
  abiDescriptions.set(address, response.data);
  refreshAbiAddressOptions(address);
  abiAddressNewRow.hidden = true;
  abiAddressNew.value = "";
  setStatus("#abi-status", `${response.data.functions.length} callable function(s) saved locally.`, "success");
  if (target.value.toLowerCase() === address) renderFunctions();
}

async function removeAbi(): Promise<void> {
  const address = activeAbiAddress();
  if (!address || !abiStore.get(address)) {
    setStatus("#abi-status", "Pick a saved contract to remove.", "error");
    return;
  }
  const response = await client.request<void>("removeAbi", { address });
  if (!response.ok) {
    setStatus("#abi-status", failureText(response.error), "error");
    return;
  }
  abiStore.remove(address);
  abiDescriptions.delete(address);
  abiJson.value = "";
  refreshAbiAddressOptions("");
  setStatus("#abi-status", "ABI removed from this browser.", "success");
  renderFunctions();
}

function renderFunctions(): void {
  functionSelect.replaceChildren();
  const description = abiDescriptions.get(target.value);
  if (!description?.functions.length) {
    functionSelect.add(new Option("Add an ABI for this target", ""));
  } else {
    for (const item of description.functions) {
      functionSelect.add(new Option(`${item.signature} · ${item.stateMutability}`, item.signature));
    }
  }
  renderArguments();
}

function selectedFunction(): AbiFunction | undefined {
  return abiDescriptions
    .get(target.value)
    ?.functions.find((item) => item.signature === functionSelect.value);
}

function renderArguments(): void {
  const container = element<HTMLElement>("#arguments");
  container.replaceChildren();
  syncExecution();
  selectedFunction()?.inputs.forEach((input, index) => {
    const label = document.createElement("label");
    label.textContent = input.name || `Argument ${index + 1}`;
    const hint = document.createElement("span");
    hint.className = "type-hint";
    hint.textContent = input.kind;
    const field = document.createElement("input");
    field.dataset.argument = String(index);
    field.spellcheck = false;
    field.placeholder = exampleFor(input.kind);
    label.append(hint, field);
    container.append(label);
  });
}

function setRawMode(enabled: boolean): void {
  rawMode = enabled;
  element<HTMLElement>("#abi-call-fields").hidden = enabled;
  element<HTMLElement>("#raw-call-fields").hidden = !enabled;
  element<HTMLElement>("#abi-mode").classList.toggle("active", !enabled);
  element<HTMLElement>("#raw-mode").classList.toggle("active", enabled);
  element<HTMLElement>("#abi-mode").setAttribute("aria-pressed", String(!enabled));
  element<HTMLElement>("#raw-mode").setAttribute("aria-pressed", String(enabled));
  syncExecution();
}

async function execute(): Promise<void> {
  if (executing || !target.value) return;
  const signature = functionSelect.value;
  if (!rawMode && !signature) {
    setStatus("#call-status", "Select a function or switch to raw calldata.", "error");
    return;
  }
  const executedOutputs = rawMode
    ? []
    : (selectedFunction()?.outputs ?? []).map((output) => ({ ...output }));
  const common = {
    target: target.value,
    caller: element<HTMLInputElement>("#caller").value.trim(),
    value: element<HTMLInputElement>("#value").value.trim(),
    gasLimit: Number(element<HTMLInputElement>("#gas-limit").value),
  };
  const payload = rawMode
    ? { ...common, mode: "raw", calldata: element<HTMLTextAreaElement>("#calldata").value.trim() }
    : {
        ...common,
        mode: "abi",
        signature,
        arguments: Array.from(document.querySelectorAll<HTMLInputElement>("[data-argument]"))
          .map((field) => field.value),
      };
  setStatus("#call-status", "Executing in the Web Worker…", "working");
  executing = true;
  syncExecution();
  const runButton = element<HTMLButtonElement>("#execute");
  runButton.textContent = "Running…";
  element<HTMLElement>(".result-panel").setAttribute("aria-busy", "true");
  const response = await client.request<CallResult>("execute", payload);
  executing = false;
  runButton.textContent = "Run call ↵";
  element<HTMLElement>(".result-panel").setAttribute("aria-busy", "false");
  syncExecution();
  if (!response.ok) {
    setStatus("#call-status", failureText(response.error), "error");
    showFailure(response.error);
    return;
  }
  setStatus("#call-status", "Execution finished without changing snapshot state.", "success");
  showResult(response.data, executedOutputs);
}

function showResult(result: CallResult, outputs: AbiParameter[]): void {
  const badge = element<HTMLElement>("#result-badge");
  badge.className = `result-badge ${result.status}`;
  badge.textContent = result.status;
  element<HTMLElement>("#gas-used").textContent = result.gasUsed.toLocaleString();
  element<HTMLElement>("#decoded-count").textContent = result.decoded ? String(result.decoded.length) : "—";
  element<HTMLElement>("#result-calldata").textContent = result.calldata;
  element<HTMLElement>("#result-output").textContent = result.output;
  renderDecodedTable(result, outputs);
}

function renderDecodedTable(result: CallResult, outputs: AbiParameter[]): void {
  const container = element<HTMLElement>("#result-decoded");
  container.replaceChildren();
  if (!result.decoded) {
    container.append(placeholder(result.reason ?? "No decoded values."));
    return;
  }
  const table = document.createElement("table");
  table.className = "decoded-table";

  const thead = document.createElement("thead");
  const headRow = document.createElement("tr");
  for (const heading of ["Name", "Type", "Value"]) {
    const th = document.createElement("th");
    th.scope = "col";
    th.textContent = heading;
    headRow.append(th);
  }
  thead.append(headRow);
  table.append(thead);

  const tbody = document.createElement("tbody");
  result.decoded.forEach((value, index) => {
    const row = document.createElement("tr");

    const name = document.createElement("th");
    name.scope = "row";
    name.textContent = outputs[index]?.name?.trim() || `output[${index}]`;
    row.append(name);

    const kind = document.createElement("td");
    kind.className = "decoded-kind";
    kind.textContent = outputs[index]?.kind ?? "—";
    row.append(kind);

    const cell = document.createElement("td");
    cell.className = "decoded-value";
    cell.textContent = value;
    row.append(cell);

    tbody.append(row);
  });
  table.append(tbody);
  container.append(table);
}

function placeholder(text: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "decoded-placeholder";
  p.textContent = text;
  return p;
}

function showFailure(failure: ApiFailure): void {
  const badge = element<HTMLElement>("#result-badge");
  badge.className = "result-badge error";
  badge.textContent = "error";
  element<HTMLElement>("#gas-used").textContent = "—";
  element<HTMLElement>("#decoded-count").textContent = "—";
  element<HTMLElement>("#result-calldata").textContent = "0x";
  element<HTMLElement>("#result-output").textContent = "0x";
  const container = element<HTMLElement>("#result-decoded");
  container.replaceChildren();
  container.append(placeholder(failureText(failure)));
}

function setStatus(selector: string, message: string, kind: "working" | "success" | "error"): void {
  const status = element<HTMLElement>(selector);
  status.className = `status ${kind}`;
  status.textContent = message;
}

function failureText(failure: ApiFailure): string {
  return [failure.message, ...failure.causes].filter(Boolean).join(" · ");
}

function exampleFor(kind: string): string {
  if (kind === "address") return "0x…";
  if (kind === "bool") return "true";
  if (kind.startsWith("bytes")) return "0x…";
  if (kind.endsWith("[]")) return "[value, value]";
  return "0";
}
