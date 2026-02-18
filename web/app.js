"use strict";

let schema = null;
let currentMethod = null;

async function init() {
  const resp = await fetch("/openrpc.json");
  schema = await resp.json();
  loadConfig();
  pushConfig();
  renderSidebar();
  document.getElementById("search").addEventListener("input", filterMethods);
  document.getElementById("cfg-connect").addEventListener("click", saveAndPushConfig);
  document.getElementById("execute").addEventListener("click", execute);
}

function loadConfig() {
  const saved = localStorage.getItem("rpc-config");
  if (!saved) return;
  try {
    const cfg = JSON.parse(saved);
    if (cfg.url) document.getElementById("cfg-url").value = cfg.url;
    if (cfg.user) document.getElementById("cfg-user").value = cfg.user;
    if (cfg.password) document.getElementById("cfg-password").value = cfg.password;
  } catch (_) {}
}

function getConfig() {
  return {
    url: document.getElementById("cfg-url").value,
    user: document.getElementById("cfg-user").value,
    password: document.getElementById("cfg-password").value,
  };
}

function saveAndPushConfig() {
  const cfg = getConfig();
  localStorage.setItem("rpc-config", JSON.stringify(cfg));
  pushConfig();
}

async function pushConfig() {
  await fetch("/config?" + encodeURIComponent(JSON.stringify(getConfig())));
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
    details.open = true;
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

init();
