"use strict";

let schema = null;
let currentMethod = null;
let dashInterval = null;
let lastPeers = [];

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
  document.getElementById("header-title").addEventListener("click", showDashboard);
  document.getElementById("cfg-poll-interval").addEventListener("change", () => {
    saveConfig();
    startDashboardPolling();
  });
  startDashboardPolling();
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
    if (cfg.pollInterval) document.getElementById("cfg-poll-interval").value = cfg.pollInterval;
    if (cfg.zmq_address) document.getElementById("cfg-zmq").value = cfg.zmq_address;
  } catch (_) {}
}

function getConfig() {
  return {
    url: document.getElementById("cfg-url").value,
    user: document.getElementById("cfg-user").value,
    password: document.getElementById("cfg-password").value,
    wallet: document.getElementById("cfg-wallet").value,
    pollInterval: document.getElementById("cfg-poll-interval").value,
    zmq_address: document.getElementById("cfg-zmq").value,
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
  if (!document.getElementById("dashboard").hidden) startDashboardPolling();
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

  document.getElementById("dashboard").hidden = true;
  document.getElementById("peer-view").hidden = true;
  stopDashboardPolling();
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

// --- Dashboard ---

function showDashboard() {
  document.getElementById("method-view").hidden = true;
  document.getElementById("peer-view").hidden = true;
  document.getElementById("dashboard").hidden = false;
  document.querySelectorAll("#method-list .method.active").forEach((el) => el.classList.remove("active"));
  currentMethod = null;
  startDashboardPolling();
}

function startDashboardPolling() {
  stopDashboardPolling();
  fetchDashboard();
  const ms = Number(document.getElementById("cfg-poll-interval").value) * 1000;
  dashInterval = setInterval(fetchDashboard, ms);
  startZmqPolling();
}

function stopDashboardPolling() {
  if (dashInterval) {
    clearInterval(dashInterval);
    dashInterval = null;
  }
  stopZmqPolling();
}

async function fetchDashboard() {
  try {
    const [chain, net, mempool, peers, up] = await Promise.all([
      rpcCall("getblockchaininfo", []),
      rpcCall("getnetworkinfo", []),
      rpcCall("getmempoolinfo", []),
      rpcCall("getpeerinfo", []),
      rpcCall("uptime", []),
    ]);
    if (chain.result) renderChain(chain.result, up.result);
    if (mempool.result) renderMempool(mempool.result);
    if (net.result) renderNetwork(net.result);
    if (peers.result) renderPeers(peers.result);
    updateStatus(true);
  } catch (_) {
    updateStatus(false);
  }
}

function esc(s) {
  const d = document.createElement("span");
  d.textContent = s;
  return d.innerHTML;
}

function dd(label, value) {
  return `<dt>${esc(label)}</dt><dd>${esc(String(value))}</dd>`;
}

function formatDuration(secs) {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const parts = [];
  if (d) parts.push(d + "d");
  if (h) parts.push(h + "h");
  parts.push(m + "m");
  return parts.join(" ");
}

function formatBytes(bytes) {
  if (bytes < 1e6) return (bytes / 1e3).toFixed(1) + " KB";
  if (bytes < 1e9) return (bytes / 1e6).toFixed(1) + " MB";
  return (bytes / 1e9).toFixed(2) + " GB";
}

function renderChain(c, uptime) {
  const dl = document.querySelector("#dash-chain dl");
  let html = "";
  html += dd("Chain", c.chain);
  html += dd("Blocks", c.blocks.toLocaleString());
  html += dd("Headers", c.headers.toLocaleString());
  html += dd("Difficulty", Number(c.difficulty).toExponential(3));
  html += dd("Progress", (c.verificationprogress * 100).toFixed(4) + "%");
  html += dd("Pruned", c.pruned ? "yes" : "no");
  html += dd("Disk size", formatBytes(c.size_on_disk));
  if (uptime != null) html += dd("Uptime", formatDuration(uptime));
  dl.innerHTML = html;
}

function renderMempool(m) {
  const dl = document.querySelector("#dash-mempool dl");
  let html = "";
  html += dd("Transactions", m.size.toLocaleString());
  html += dd("Size", formatBytes(m.bytes));
  html += dd("Memory usage", formatBytes(m.usage));
  html += dd("Min fee", m.mempoolminfee + " BTC/kvB");
  dl.innerHTML = html;
}

function renderNetwork(n) {
  const dl = document.querySelector("#dash-network dl");
  let html = "";
  html += dd("User agent", n.subversion);
  html += dd("Protocol", n.protocolversion);
  html += dd("Connections", n.connections + " (" + n.connections_in + " in / " + n.connections_out + " out)");
  if (n.localservicesnames) html += dd("Services", n.localservicesnames.join(", "));
  if (n.warnings) html += dd("Warnings", n.warnings);
  dl.innerHTML = html;
}

function renderPeers(peers) {
  lastPeers = peers;
  const tbody = document.querySelector("#dash-peer-table tbody");
  let html = "";
  for (const p of peers) {
    html += '<tr class="peer-row" data-peer-id="' + p.id + '">';
    html += "<td>" + esc(p.addr) + "</td>";
    html += "<td>" + esc(p.subver) + "</td>";
    html += "<td>" + (p.inbound ? "in" : "out") + "</td>";
    html += "<td>" + (p.pingtime != null ? (p.pingtime * 1000).toFixed(0) + " ms" : "–") + "</td>";
    html += "</tr>";
  }
  tbody.innerHTML = html;
  for (const row of tbody.querySelectorAll(".peer-row")) {
    row.addEventListener("click", () => {
      const id = Number(row.dataset.peerId);
      const peer = lastPeers.find(p => p.id === id);
      if (peer) showPeerDetail(peer);
    });
  }
}

function showPeerDetail(peer) {
  document.getElementById("dashboard").hidden = true;
  stopDashboardPolling();
  document.getElementById("method-view").hidden = true;
  document.getElementById("peer-view").hidden = false;
  document.getElementById("peer-view-title").textContent = peer.addr;
  const dl = document.getElementById("peer-view-dl");
  let html = "";
  for (const [key, val] of Object.entries(peer)) {
    const display = typeof val === "object" ? JSON.stringify(val, null, 2) : String(val);
    html += dd(key, display);
  }
  dl.innerHTML = html;
}

// --- ZMQ feed ---

let zmqInterval = null;

function startZmqPolling() {
  stopZmqPolling();
  fetchZmq();
  zmqInterval = setInterval(fetchZmq, 2000);
}

function stopZmqPolling() {
  if (zmqInterval) {
    clearInterval(zmqInterval);
    zmqInterval = null;
  }
}

async function fetchZmq() {
  try {
    const resp = await fetch("/zmq/messages");
    const data = await resp.json();
    renderZmq(data);
  } catch (_) {
    document.getElementById("dash-zmq").hidden = true;
  }
}

function reverseHex(hex) {
  return hex.match(/.{2}/g).reverse().join("");
}

function formatUnixTime(secs) {
  const d = new Date(secs * 1000);
  return d.toTimeString().slice(0, 8);
}

function colorHexBytes(hex) {
  let html = "";
  for (let i = 0; i < hex.length; i += 2) {
    const pair = hex.slice(i, i + 2);
    const val = parseInt(pair, 16);
    const hue = Math.round(val * 360 / 256);
    html += '<span style="color:hsl(' + hue + ',70%,65%)">' + esc(pair) + '</span>';
  }
  return html;
}

function zmqTopicClass(topic) {
  if (topic === "hashblock" || topic === "rawblock") return "zmq-topic-block";
  if (topic === "hashtx" || topic === "rawtx") return "zmq-topic-tx";
  return "zmq-topic-meta";
}

function renderZmq(data) {
  const section = document.getElementById("dash-zmq");
  if (!data.connected || data.messages.length === 0) {
    section.hidden = true;
    return;
  }
  section.hidden = false;
  const feed = document.getElementById("dash-zmq-feed");
  let html = "";
  for (let i = data.messages.length - 1; i >= 0; i--) {
    const msg = data.messages[i];
    const time = formatUnixTime(msg.timestamp);
    const topic = msg.topic;
    const topicCls = zmqTopicClass(topic);
    let dataHtml;
    if (topic === "hashblock" || topic === "hashtx") {
      dataHtml = colorHexBytes(reverseHex(msg.body_hex));
    } else if (topic === "rawblock" || topic === "rawtx") {
      dataHtml = esc(formatBytes(msg.body_size));
    } else {
      dataHtml = colorHexBytes(msg.body_hex);
    }
    html += '<div class="zmq-row">'
      + '<span class="zmq-time">' + esc(time) + '</span>'
      + '<span class="zmq-topic ' + topicCls + '">' + esc(topic) + '</span>'
      + '<span class="zmq-data">' + dataHtml + '</span>'
      + '</div>';
  }
  feed.innerHTML = html;
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
