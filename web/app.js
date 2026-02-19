"use strict";

let schema = null;
let currentMethod = null;
let dashTimer = null;
let lastPeers = [];
let audioEnabled = true;
let zmqConnected = false;
let dashboardFetchInFlight = false;
let dashboardFetchQueued = false;
let zmqRefreshTimer = null;
let zmqMessageLookup = new Map();
let dashboardPollingGeneration = 0;
let zmqPollingGeneration = 0;
let pendingDashboardParts = new Set();
let peerById = new Map();
let peerRows = new Map();
let lastZmqCursor = 0;
let lastPeersRefreshMs = 0;
const ZMQ_FAST_POLL_MS = 250;
const ZMQ_SLOW_POLL_MS = 2000;
const DASHBOARD_ZMQ_FALLBACK_MS = 15_000;
const DASHBOARD_PART_DEBOUNCE_MS = 250;
const PEERS_REFRESH_MIN_MS = 10_000;
const ZMQ_FEED_MAX_ROWS = 200;
const ZMQ_LONG_POLL_WAIT_MS = 5_000;

function encodeHeaderJson(value) {
  return encodeURIComponent(JSON.stringify(value));
}

async function init() {
  const resp = await fetch("/openrpc.json");
  schema = await resp.json();
  try {
    const r = await fetch("/features");
    const j = await r.json();
    audioEnabled = j.audio !== false;
  } catch (_) {}
  loadConfig();
  await pushConfig();
  const ok = await loadWallets();
  updateStatus(ok);
  renderSidebar();
  document.getElementById("search").addEventListener("input", filterMethods);
  document.getElementById("cfg-toggle").addEventListener("click", toggleConfig);
  document.getElementById("cfg-connect").addEventListener("click", connectClicked);
  document.getElementById("cfg-wallet").addEventListener("change", walletChanged);
  document.getElementById("cfg-zmq-buffer-limit").addEventListener("change", zmqBufferLimitChanged);
  document.getElementById("execute").addEventListener("click", execute);
  document.getElementById("header-title").addEventListener("click", showDashboard);
  document.getElementById("cfg-poll-interval").addEventListener("change", () => {
    saveConfig();
    startDashboardPolling();
  });
  document.getElementById("cfg-url").addEventListener("input", clearUrlError);
  initPeerTableClick();
  initZmqFeedClick();
  startDashboardPolling();
  if (audioEnabled) {
    initMusic();
  } else {
    const bar = document.getElementById("music-bar");
    if (bar) bar.hidden = true;
  }
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
    if (cfg.zmq_buffer_limit) document.getElementById("cfg-zmq-buffer-limit").value = cfg.zmq_buffer_limit;
  } catch (_) {}
}

function getConfig() {
  const zmqBufferLimit = Number(document.getElementById("cfg-zmq-buffer-limit").value);
  return {
    url: document.getElementById("cfg-url").value,
    user: document.getElementById("cfg-user").value,
    password: document.getElementById("cfg-password").value,
    wallet: document.getElementById("cfg-wallet").value,
    pollInterval: document.getElementById("cfg-poll-interval").value,
    zmq_address: document.getElementById("cfg-zmq").value,
    zmq_buffer_limit: Number.isFinite(zmqBufferLimit) ? zmqBufferLimit : 1000,
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
  const cfg = getConfig();
  try {
    const resp = await fetch("/config", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-app-json": encodeHeaderJson(cfg),
      },
      body: JSON.stringify(cfg),
    });
    return await resp.json();
  } catch (_) {
    return { ok: false };
  }
}

function toggleConfig() {
  document.getElementById("config").classList.toggle("collapsed");
}

function clearUrlError() {
  const input = document.getElementById("cfg-url");
  const err = document.getElementById("cfg-url-error");
  input.classList.remove("cfg-error");
  err.hidden = true;
  err.textContent = "";
}

function showUrlError(msg) {
  const input = document.getElementById("cfg-url");
  const err = document.getElementById("cfg-url-error");
  input.classList.add("cfg-error");
  err.textContent = msg;
  err.hidden = false;
}

async function connectClicked() {
  const cfgResp = await pushConfig();
  if (cfgResp.insecure_blocked) {
    showUrlError("Non-local RPC address blocked. Set DANGER_INSECURE_RPC=1 to override.");
    return;
  }
  clearUrlError();
  saveConfig();
  const ok = await loadWallets();
  updateStatus(ok);
  if (!document.getElementById("dashboard").hidden) startDashboardPolling();
}

async function walletChanged() {
  saveConfig();
  await pushConfig();
}

async function zmqBufferLimitChanged() {
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
  document.getElementById("execute").hidden = false;
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
  const payload = { method, params };
  const resp = await fetch("/rpc", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-app-json": encodeHeaderJson(payload),
    },
    body: JSON.stringify(payload),
  });
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
  dashboardPollingGeneration += 1;
  stopDashboardPolling();
  const generation = dashboardPollingGeneration;
  fetchDashboard();
  scheduleDashboardPoll(generation);
  startZmqPolling(generation);
}

function stopDashboardPolling() {
  dashboardPollingGeneration += 1;
  if (dashTimer) {
    clearTimeout(dashTimer);
    dashTimer = null;
  }
  stopZmqPolling();
}

function dashboardPollMs() {
  const configured = Math.max(1, Number(document.getElementById("cfg-poll-interval").value) || 5) * 1000;
  return zmqConnected ? Math.max(configured, DASHBOARD_ZMQ_FALLBACK_MS) : configured;
}

function scheduleDashboardPoll(generation) {
  if (dashTimer) clearTimeout(dashTimer);
  dashTimer = setTimeout(async () => {
    if (generation !== dashboardPollingGeneration) return;
    await fetchDashboard();
    if (generation !== dashboardPollingGeneration) return;
    scheduleDashboardPoll(generation);
  }, dashboardPollMs());
}

function requestDashboardRefreshSoon() {
  if (zmqRefreshTimer) return;
  zmqRefreshTimer = setTimeout(async () => {
    zmqRefreshTimer = null;
    await flushDashboardPartRefreshes();
  }, DASHBOARD_PART_DEBOUNCE_MS);
}

function dashboardVisible() {
  return !document.getElementById("dashboard").hidden;
}

function queueDashboardPartRefresh(parts) {
  if (!dashboardVisible()) return;
  for (const part of parts) pendingDashboardParts.add(part);
  requestDashboardRefreshSoon();
}

function deriveDashboardParts(messages) {
  const parts = new Set();
  for (const msg of messages) {
    if (msg.topic === "hashblock" || msg.topic === "rawblock") {
      parts.add("chain");
      parts.add("mempool");
    } else if (msg.topic === "hashtx" || msg.topic === "rawtx" || msg.topic === "sequence") {
      parts.add("mempool");
    }
  }
  return parts;
}

async function flushDashboardPartRefreshes() {
  if (!dashboardVisible() || pendingDashboardParts.size === 0) return;
  if (dashboardFetchInFlight) return;
  const parts = new Set(pendingDashboardParts);
  pendingDashboardParts.clear();
  const tasks = [];
  if (parts.has("chain")) {
    tasks.push((async () => {
      const [chain, uptime] = await Promise.all([
        rpcCall("getblockchaininfo", []),
        rpcCall("uptime", []),
      ]);
      if (chain.result) renderChain(chain.result, uptime.result);
    })());
  }
  if (parts.has("mempool")) {
    tasks.push((async () => {
      const mempool = await rpcCall("getmempoolinfo", []);
      if (mempool.result) renderMempool(mempool.result);
    })());
  }
  const now = Date.now();
  if (parts.has("peers") && (now - lastPeersRefreshMs >= PEERS_REFRESH_MIN_MS)) {
    tasks.push((async () => {
      const peers = await rpcCall("getpeerinfo", []);
      if (peers.result) {
        renderPeers(peers.result);
        lastPeersRefreshMs = Date.now();
      }
    })());
  }
  if (tasks.length === 0) return;
  try {
    await Promise.all(tasks);
    updateStatus(true);
  } catch (_) {
    updateStatus(false);
  }
}

async function fetchDashboard() {
  if (dashboardFetchInFlight) {
    dashboardFetchQueued = true;
    return;
  }
  dashboardFetchInFlight = true;
  try {
    const [chain, net, mempool, peers, up, totals] = await Promise.all([
      rpcCall("getblockchaininfo", []),
      rpcCall("getnetworkinfo", []),
      rpcCall("getmempoolinfo", []),
      rpcCall("getpeerinfo", []),
      rpcCall("uptime", []),
      rpcCall("getnettotals", []),
    ]);
    if (chain.result) renderChain(chain.result, up.result);
    if (mempool.result) renderMempool(mempool.result);
    if (net.result) renderNetwork(net.result);
    if (totals.result) renderNetTotals(totals.result);
    if (peers.result) {
      renderPeers(peers.result);
      lastPeersRefreshMs = Date.now();
    }
    pendingDashboardParts.clear();
    updateStatus(true);
  } catch (_) {
    updateStatus(false);
  } finally {
    dashboardFetchInFlight = false;
    if (dashboardFetchQueued) {
      dashboardFetchQueued = false;
      fetchDashboard();
    }
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

function renderNetTotals(t) {
  const dl = document.querySelector("#dash-nettotals dl");
  let html = "";
  html += dd("Received", formatBytes(t.totalbytesrecv));
  html += dd("Sent", formatBytes(t.totalbytessent));
  const up = t.uploadtarget;
  if (up && up.target > 0) {
    html += dd("Upload target", formatBytes(up.target));
    html += dd("Left in cycle", formatBytes(up.bytes_left_in_cycle));
    html += dd("Serve historical", up.serve_historical_blocks ? "yes" : "no");
  }
  dl.innerHTML = html;
}

function renderPeers(peers) {
  lastPeers = peers;
  peerById = new Map(peers.map((p) => [p.id, p]));
  const tbody = document.querySelector("#dash-peer-table tbody");
  const seen = new Set();
  for (const p of peers) {
    seen.add(p.id);
    let row = peerRows.get(p.id);
    if (!row) {
      row = document.createElement("tr");
      row.className = "peer-row";
      row.dataset.peerId = String(p.id);
      row.appendChild(document.createElement("td"));
      row.appendChild(document.createElement("td"));
      row.appendChild(document.createElement("td"));
      row.appendChild(document.createElement("td"));
      peerRows.set(p.id, row);
    }
    const direction = p.inbound ? "in" : "out";
    const ping = p.pingtime != null ? (p.pingtime * 1000).toFixed(0) + " ms" : "–";
    if (row.children[0].textContent !== p.addr) row.children[0].textContent = p.addr;
    if (row.children[1].textContent !== p.subver) row.children[1].textContent = p.subver;
    if (row.children[2].textContent !== direction) row.children[2].textContent = direction;
    row.children[2].className = p.inbound ? "peer-in" : "peer-out";
    if (row.children[3].textContent !== ping) row.children[3].textContent = ping;
    tbody.appendChild(row);
  }
  for (const [id, row] of peerRows) {
    if (seen.has(id)) continue;
    row.remove();
    peerRows.delete(id);
  }
}

function initPeerTableClick() {
  const tbody = document.querySelector("#dash-peer-table tbody");
  tbody.addEventListener("click", (ev) => {
    const row = ev.target.closest(".peer-row");
    if (!row) return;
    const id = Number(row.dataset.peerId);
    const peer = peerById.get(id) || lastPeers.find((p) => p.id === id);
    if (peer) showPeerDetail(peer);
  });
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

async function showZmqRpcResult(title, description, run) {
  document.getElementById("dashboard").hidden = true;
  stopDashboardPolling();
  document.getElementById("peer-view").hidden = true;
  document.getElementById("method-view").hidden = false;
  document.querySelectorAll("#method-list .method.active").forEach((el) => el.classList.remove("active"));
  currentMethod = null;

  document.getElementById("execute").hidden = true;
  document.getElementById("method-name").textContent = title;
  document.getElementById("method-desc").textContent = description;
  document.getElementById("param-form").innerHTML = "";
  const result = document.getElementById("result");
  result.classList.remove("error");
  result.classList.add("visible");
  result.textContent = "Loading...";

  try {
    const resp = await run();
    result.classList.remove("error");
    if (resp && resp.error) {
      result.classList.add("error");
      result.textContent = JSON.stringify(resp.error, null, 2);
    } else {
      result.textContent = JSON.stringify(resp && resp.result !== undefined ? resp.result : resp, null, 2);
    }
  } catch (e) {
    result.classList.add("error");
    result.textContent = String(e);
  }
}

// --- ZMQ feed ---

let zmqTimer = null;

function stopZmqPolling() {
  zmqPollingGeneration += 1;
  if (zmqTimer) {
    clearTimeout(zmqTimer);
    zmqTimer = null;
  }
  if (zmqRefreshTimer) {
    clearTimeout(zmqRefreshTimer);
    zmqRefreshTimer = null;
  }
}

function startZmqPolling(dashboardGeneration) {
  stopZmqPolling();
  zmqPollingGeneration = dashboardGeneration;
  pollZmqLoop(zmqPollingGeneration);
}

function setZmqConnected(next) {
  if (zmqConnected === next) return;
  zmqConnected = next;
  scheduleDashboardPoll(dashboardPollingGeneration);
}

async function pollZmqLoop(generation) {
  if (generation !== zmqPollingGeneration) return;
  const data = await fetchZmq();
  if (generation !== zmqPollingGeneration) return;
  const connected = !!(data && data.connected);
  setZmqConnected(connected);
  const delay = connected ? 0 : ZMQ_SLOW_POLL_MS;
  zmqTimer = setTimeout(() => pollZmqLoop(generation), delay);
}

async function fetchZmq() {
  try {
    const waitMs = zmqConnected ? ZMQ_LONG_POLL_WAIT_MS : 0;
    const resp = await fetch(`/zmq/messages?since=${encodeURIComponent(String(lastZmqCursor))}&wait_ms=${waitMs}`);
    const data = await resp.json();
    if (typeof data.cursor === "number" && Number.isFinite(data.cursor)) {
      lastZmqCursor = data.cursor;
    }
    if (data.truncated) {
      clearZmqFeed();
    }
    if (Array.isArray(data.messages) && data.messages.length > 0) {
      renderZmq(data);
      queueDashboardPartRefresh(deriveDashboardParts(data.messages));
    }
    if (!data.connected) renderZmq(data);
    return data;
  } catch (_) {
    clearZmqFeed();
    return null;
  }
}

function formatUnixTime(secs) {
  const d = new Date(secs * 1000);
  return d.toTimeString().slice(0, 8);
}

function zmqTopicClass(topic) {
  if (topic === "hashblock" || topic === "rawblock") return "zmq-topic-block";
  if (topic === "hashtx" || topic === "rawtx") return "zmq-topic-tx";
  return "zmq-topic-meta";
}

function zmqRowAction(msg) {
  const hash = msg.event_hash;
  if (msg.topic === "hashblock" && hash) {
    return {
      title: `ZMQ hashblock ${hash}`,
      description: "Triggered by ZMQ hashblock. RPC: getblockheader <hash> true",
      run: () => rpcCall("getblockheader", [hash, true]),
    };
  }
  if (msg.topic === "hashtx" && hash) {
    return {
      title: `ZMQ hashtx ${hash}`,
      description: "Triggered by ZMQ hashtx. RPC: getrawtransaction <hash> 1",
      run: () => rpcCall("getrawtransaction", [hash, 1]),
    };
  }
  if (msg.topic === "rawblock" && hash) {
    return {
      title: `ZMQ rawblock ${hash}`,
      description: "Triggered by ZMQ rawblock. RPC: getblock <hash> 1",
      run: () => rpcCall("getblock", [hash, 1]),
    };
  }
  if (msg.topic === "rawtx") {
    const qs = `timestamp=${encodeURIComponent(String(msg.timestamp))}&sequence=${encodeURIComponent(String(msg.sequence))}`;
    return {
      title: `ZMQ rawtx seq=${msg.sequence}`,
      description: "Triggered by ZMQ rawtx. RPC: decoderawtransaction <rawtx>",
      run: () => fetch(`/zmq/decode-rawtx?${qs}`).then((r) => r.json()),
    };
  }
  return null;
}

function handleZmqRowClick(id) {
  const msg = zmqMessageLookup.get(id);
  if (!msg) return;
  const action = zmqRowAction(msg);
  if (!action) return;
  showZmqRpcResult(action.title, action.description, action.run);
}

function initZmqFeedClick() {
  const feed = document.getElementById("dash-zmq-feed");
  feed.addEventListener("click", (ev) => {
    const row = ev.target.closest(".zmq-row.zmq-clickable");
    if (!row) return;
    handleZmqRowClick(row.dataset.zmqId);
  });
}

function buildZmqRow(msg) {
  const time = formatUnixTime(msg.timestamp);
  const topic = msg.topic;
  const topicCls = zmqTopicClass(topic);
  const rowId = String(msg.cursor ?? `${msg.timestamp}-${msg.sequence}-${topic}`);
  const action = zmqRowAction(msg);
  zmqMessageLookup.set(rowId, msg);

  let dataHtml;
  if (msg.event_hash) {
    dataHtml = esc(msg.event_hash);
  } else if (topic === "rawblock" || topic === "rawtx") {
    dataHtml = esc(formatBytes(msg.body_size));
  } else {
    dataHtml = esc(msg.body_hex);
  }

  const row = document.createElement("div");
  row.className = "zmq-row" + (action ? " zmq-clickable" : "");
  row.dataset.zmqId = rowId;
  row.innerHTML =
    '<span class="zmq-time">' + esc(time) + '</span>'
    + '<span class="zmq-topic ' + topicCls + '">' + esc(topic) + '</span>'
    + '<span class="zmq-data">' + dataHtml + "</span>";
  return row;
}

function renderZmq(data) {
  const section = document.getElementById("dash-zmq");
  const feed = document.getElementById("dash-zmq-feed");
  if (!data.connected) {
    section.hidden = true;
    feed.textContent = "";
    zmqMessageLookup = new Map();
    return;
  }
  if (!Array.isArray(data.messages) || data.messages.length === 0) {
    section.hidden = true;
    if (!data.connected) {
      feed.textContent = "";
      zmqMessageLookup = new Map();
    }
    return;
  }
  section.hidden = false;
  for (let i = 0; i < data.messages.length; i++) {
    const row = buildZmqRow(data.messages[i]);
    feed.prepend(row);
  }
  while (feed.children.length > ZMQ_FEED_MAX_ROWS) {
    const stale = feed.lastElementChild;
    if (!stale) break;
    if (stale.dataset.zmqId) zmqMessageLookup.delete(stale.dataset.zmqId);
    stale.remove();
  }
}

function clearZmqFeed() {
  const section = document.getElementById("dash-zmq");
  const feed = document.getElementById("dash-zmq-feed");
  section.hidden = true;
  feed.textContent = "";
  zmqMessageLookup = new Map();
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
