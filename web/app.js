"use strict";

let schema = null;
let currentMethod = null;

async function init() {
  const resp = await fetch("/openrpc.json");
  schema = await resp.json();
  loadConfig();
  await pushConfig();
  const ok = await loadWallets();
  updateStatus(ok);
  renderSidebar();
  document.getElementById("search").addEventListener("input", filterMethods);
  document.getElementById("cfg-toggle").addEventListener("click", toggleConfig);
  document.getElementById("cfg-connect").addEventListener("click", connectClicked);
  document.getElementById("cfg-wallet").addEventListener("change", walletChanged);
  document.getElementById("execute").addEventListener("click", execute);
}

function loadConfig() {
  const saved = localStorage.getItem("rpc-config");
  if (!saved) return;
  try {
    const cfg = JSON.parse(saved);
    if (cfg.url) document.getElementById("cfg-url").value = cfg.url;
    if (cfg.user) document.getElementById("cfg-user").value = cfg.user;
    if (cfg.password) {
      document.getElementById("cfg-password").value = cfg.password;
      document.getElementById("cfg-save-pw").checked = true;
    }
    if (cfg.wallet) document.getElementById("cfg-wallet").value = cfg.wallet;
  } catch (_) {}
}

function getConfig() {
  return {
    url: document.getElementById("cfg-url").value,
    user: document.getElementById("cfg-user").value,
    password: document.getElementById("cfg-password").value,
    wallet: document.getElementById("cfg-wallet").value,
  };
}

function saveConfig() {
  const cfg = getConfig();
  const savePw = document.getElementById("cfg-save-pw").checked;
  if (!savePw) {
    const { password, ...safe } = cfg;
    localStorage.setItem("rpc-config", JSON.stringify(safe));
  } else {
    localStorage.setItem("rpc-config", JSON.stringify(cfg));
  }
}

async function pushConfig() {
  await fetch("/config?" + encodeURIComponent(JSON.stringify(getConfig())));
}

function toggleConfig() {
  document.getElementById("config").classList.toggle("collapsed");
}

async function connectClicked() {
  saveConfig();
  await pushConfig();
  const ok = await loadWallets();
  updateStatus(ok);
}

async function walletChanged() {
  saveConfig();
  await pushConfig();
}

async function loadWallets() {
  const select = document.getElementById("cfg-wallet");
  const current = select.value;
  try {
    const resp = await rpcCall("listwallets", []);
    if (resp.error) return false;
    const wallets = resp.result;
    if (!Array.isArray(wallets)) return false;
    select.innerHTML = '<option value="">(none)</option>';
    for (const w of wallets) {
      const opt = document.createElement("option");
      opt.value = w;
      opt.textContent = w;
      select.appendChild(opt);
    }
    select.value = current;
    return true;
  } catch (_) {
    return false;
  }
}

function updateStatus(connected) {
  const dot = document.getElementById("connection-status");
  dot.classList.toggle("connected", connected);
  dot.title = connected ? "Connected" : "Disconnected";
}

function renderSidebar() {
  const groups = {};
  for (const m of schema.methods) {
    const cat = m["x-bitcoin-category"] || "other";
    if (!groups[cat]) groups[cat] = [];
    groups[cat].push(m);
  }

  const nav = document.getElementById("method-list");
  nav.innerHTML = "";

  for (const cat of Object.keys(groups).sort()) {
    const details = document.createElement("details");
    details.open = false;
    const summary = document.createElement("summary");
    summary.textContent = `${cat} (${groups[cat].length})`;
    details.appendChild(summary);

    for (const m of groups[cat]) {
      const a = document.createElement("a");
      a.className = "method";
      a.textContent = m.name;
      a.dataset.name = m.name;
      a.addEventListener("click", () => selectMethod(m));
      details.appendChild(a);
    }
    nav.appendChild(details);
  }
}

function filterMethods() {
  const q = document.getElementById("search").value.toLowerCase();
  const methods = document.querySelectorAll("#method-list .method");
  const details = document.querySelectorAll("#method-list details");

  for (const m of methods) {
    m.hidden = !m.dataset.name.includes(q);
  }
  for (const d of details) {
    const visible = d.querySelectorAll(".method:not([hidden])");
    d.hidden = visible.length === 0;
  }
}

function selectMethod(m) {
  currentMethod = m;

  document.querySelectorAll("#method-list .method.active").forEach((el) => el.classList.remove("active"));
  const link = document.querySelector(`#method-list .method[data-name="${m.name}"]`);
  if (link) link.classList.add("active");

  document.getElementById("empty-state").hidden = true;
  document.getElementById("method-view").hidden = false;
  document.getElementById("method-name").textContent = m.name;
  document.getElementById("method-desc").textContent = m.description || "";

  const form = document.getElementById("param-form");
  form.innerHTML = "";
  for (const p of m.params || []) {
    form.appendChild(buildField(p));
  }

  const result = document.getElementById("result");
  result.classList.remove("visible", "error");
  result.textContent = "";
}

function buildField(param) {
  const div = document.createElement("div");
  div.className = "field";

  const label = document.createElement("label");
  label.className = "field-label";
  label.textContent = param.name;
  if (!param.required) {
    const opt = document.createElement("span");
    opt.className = "optional";
    opt.textContent = " (optional)";
    label.appendChild(opt);
  }
  div.appendChild(label);

  if (param.description) {
    const desc = document.createElement("div");
    desc.className = "field-desc";
    desc.textContent = param.description;
    div.appendChild(desc);
  }

  const s = param.schema || {};
  let input;

  if (s.type === "boolean") {
    input = document.createElement("select");
    input.innerHTML = '<option value="">(default)</option><option value="true">true</option><option value="false">false</option>';
  } else if (s.type === "array" || s.type === "object") {
    input = document.createElement("textarea");
    input.placeholder = `JSON ${s.type}`;
  } else {
    input = document.createElement("input");
    input.type = "text";
    if (s.pattern) input.pattern = s.pattern;
    if (s.type === "number") input.placeholder = "number";
  }

  input.dataset.paramName = param.name;
  input.dataset.schemaType = s.type || "string";
  div.appendChild(input);
  return div;
}

function extractValue(input) {
  const raw = input.value.trim();
  if (raw === "") return undefined;

  const type = input.dataset.schemaType;
  if (type === "boolean") return raw === "true";
  if (type === "number") return Number(raw);
  if (type === "array" || type === "object") {
    try { return JSON.parse(raw); }
    catch (_) { return raw; }
  }
  if (raw === "true") return true;
  if (raw === "false") return false;
  if (raw !== "" && !isNaN(raw) && !isNaN(parseFloat(raw))) return Number(raw);
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === "object") return parsed;
  } catch (_) {}
  return raw;
}

async function execute() {
  if (!currentMethod) return;

  const inputs = document.querySelectorAll("#param-form [data-param-name]");
  const params = [];
  for (const input of inputs) {
    params.push(extractValue(input));
  }

  while (params.length > 0 && params[params.length - 1] === undefined) {
    params.pop();
  }
  for (let i = 0; i < params.length; i++) {
    if (params[i] === undefined) params[i] = null;
  }

  const btn = document.getElementById("execute");
  btn.disabled = true;
  btn.textContent = "Loading...";

  const result = document.getElementById("result");
  result.classList.remove("visible", "error");

  try {
    const resp = await rpcCall(currentMethod.name, params);
    result.classList.add("visible");
    if (resp.error) {
      result.classList.add("error");
      result.textContent = JSON.stringify(resp.error, null, 2);
    } else {
      result.textContent = JSON.stringify(resp.result !== undefined ? resp.result : resp, null, 2);
    }
  } catch (e) {
    result.classList.add("visible", "error");
    result.textContent = String(e);
  } finally {
    btn.disabled = false;
    btn.textContent = "Execute";
  }
}

async function rpcCall(method, params) {
  const resp = await fetch("/rpc?" + encodeURIComponent(JSON.stringify({ method, params })));
  return resp.json();
}

// --- Music player ---

function initMusic() {
  document.getElementById("music-prev").addEventListener("click", () => musicCmd("prev"));
  document.getElementById("music-play").addEventListener("click", () => musicCmd("playpause"));
  document.getElementById("music-next").addEventListener("click", () => musicCmd("next"));
  document.getElementById("music-mute").addEventListener("click", () => musicCmd("mute"));
  document.getElementById("music-volume").addEventListener("input", (e) => {
    fetch("/music/volume?" + (e.target.value / 100));
  });
  pollMusic();
  setInterval(pollMusic, 2000);
}

async function musicCmd(action) {
  await fetch("/music/" + action);
  pollMusic();
}

async function pollMusic() {
  try {
    const resp = await fetch("/music/status");
    const s = await resp.json();
    document.getElementById("music-track").textContent = s.track;
    document.getElementById("music-play").textContent = s.playing ? "\u23F8" : "\u25B6";
    document.getElementById("music-mute").textContent = s.muted ? "\uD83D\uDD07" : "\uD83D\uDD0A";
    document.getElementById("music-volume").value = Math.round(s.volume * 100);
  } catch (_) {}
}

init();
initMusic();
