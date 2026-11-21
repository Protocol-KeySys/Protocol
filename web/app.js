// app.js — key system frontend. Talks to the backend API.
// The SERVER is the source of truth for validity and time remaining;
// this file only renders what it tells us. (Anything enforced purely
// here could be patched — by design, nothing sensitive lives here.)

// Same-origin when served by the keyserver; explicit URL inside the
// packaged .exe (tauri.localhost) or any other host. Point SERVER_URL
// at the deployed Protocol instance for production builds.
const SERVER_URL = "http://127.0.0.1:8080";
const API = location.protocol.startsWith("http") &&
            !location.hostname.includes("tauri") ? "" : SERVER_URL;

const $ = (id) => document.getElementById(id);

let session = null;      // { key, username, platforms, seconds_left }
let allPlatforms = null; // full catalog, fetched lazily for the ••• view
let tickTimer = null;
let prevScreen = null;   // for ToS back navigation

// ---------- screen switching ----------

function show(id) {
  for (const s of ["lockScreen", "menuScreen", "tosScreen"])
    $(s).classList.toggle("hidden", s !== id);
}

function showTos() {
  prevScreen = $("menuScreen").classList.contains("hidden")
    ? "lockScreen" : "menuScreen";
  show("tosScreen");
}

for (const btn of document.querySelectorAll(".tos-btn"))
  btn.addEventListener("click", showTos);
$("tosBackBtn").addEventListener("click", () => show(prevScreen || "lockScreen"));

// ---------- lock screen ----------

$("keyInput").addEventListener("keydown", (e) => {
  if (e.key === "Enter") $("confirmBtn").click();
});

$("confirmBtn").addEventListener("click", async () => {
  const key = $("keyInput").value.trim();
  $("lockError").textContent = "";
  if (!key) { $("lockError").textContent = "Enter a key."; return; }

  try {
    const res = await fetch(`${API}/api/validate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ key }),
    });
    const data = await res.json();

    if (!res.ok || !data.valid) {
      $("lockError").textContent = data.error || "Invalid key.";
      return;
    }
    enterMenu(key, data);
  } catch {
    $("lockError").textContent = "Can't reach server.";
  }
});

// ---------- menu ----------

function enterMenu(key, data) {
  session = { key, ...data };
  show("menuScreen");

  $("menuUsername").textContent = data.username || "user";
  $("currentKey").textContent = key;

  renderEntitledView();
  startCountdown(data.seconds_left || 0);
}

// Entitled view: your platforms + helper tile. No back button here.
// 0 platforms  -> "+" tile (future platform search)
// 1-3 platforms -> "•••" tile (all apps)
// 4+ platforms -> tiles only
function renderEntitledView() {
  $("backBtn").classList.add("hidden");

  const grid = $("platformGrid");
  const panel = document.querySelector(".platform-panel");
  const empty = $("emptyArea");
  grid.innerHTML = "";

  const platforms = session.platforms || [];

  // No platforms: hide the dark panel entirely, show a centered +
  // tile in the parent card (future platform-search entry point).
  if (platforms.length === 0) {
    panel.classList.add("hidden");
    empty.classList.remove("hidden");
    return;
  }

  panel.classList.remove("hidden");
  empty.classList.add("hidden");

  for (const name of platforms) {
    grid.appendChild(makeTile(name));
  }

  if (platforms.length < 4) {
    const more = document.createElement("button");
    more.className = "tile tile-more";
    more.textContent = "•••";
    more.title = "All apps";
    more.addEventListener("click", renderAllAppsView);
    grid.appendChild(more);
  }
}

// All-apps view: full catalog, every platform enabled. Back button returns.
async function renderAllAppsView() {
  if (!allPlatforms) {
    try {
      const res = await fetch(`${API}/api/platforms`);
      allPlatforms = (await res.json()).platforms || [];
    } catch {
      toast("Can't reach server.");
      return;
    }
  }

  const grid = $("platformGrid");
  grid.innerHTML = "";

  for (const name of allPlatforms)
    grid.appendChild(makeTile(name));

  $("backBtn").classList.remove("hidden");
}

$("backBtn").addEventListener("click", renderEntitledView);

$("plusTile").addEventListener("click", () =>
  toast("Platform search coming soon."));

function makeTile(name) {
  const tile = document.createElement("button");
  tile.className = "tile";
  tile.textContent = name;
  tile.addEventListener("click", () => selectPlatform(name));
  return tile;
}

// ---------- countdown ----------

function startCountdown(seconds) {
  clearInterval(tickTimer);
  session.seconds_left = seconds;
  renderClock();

  tickTimer = setInterval(() => {
    session.seconds_left = Math.max(0, session.seconds_left - 1);
    renderClock();
    if (session.seconds_left === 0) {
      clearInterval(tickTimer);
      toast("Key expired.");
    }
  }, 1000);
}

function renderClock() {
  const s = session.seconds_left;
  const h = String(Math.floor(s / 3600)).padStart(3, "0");
  const m = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
  const sec = String(s % 60).padStart(2, "0");
  $("timeLeft").textContent = `${h}:${m}:${sec}`;
}

// ---------- actions ----------

async function selectPlatform(name) {
  const res = await fetch(`${API}/api/select`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ key: session.key, platform: name }),
  });
  const data = await res.json();
  toast(data.ok ? `Launching ${name}…` : (data.error || "Denied."));
}

$("renewBtn").addEventListener("click", async () => {
  const res = await fetch(`${API}/api/renew`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ key: session.key }),
  });
  const data = await res.json();
  if (data.checkpoint_required) {
    // Normal keys go through a Linkvertise/LootLabs-style checkpoint.
    // Stub until the checkpoint flow is built.
    toast("Checkpoint required — ad-link flow coming soon.");
  } else if (res.ok && data.seconds_left != null) {
    startCountdown(data.seconds_left);
    toast(data.capped ? "Time added (cap reached)." : "Time added.");
  } else {
    toast(data.error || "Renew failed.");
  }
});

let toastTimer = null;
function toast(msg) {
  const el = $("toast");
  el.textContent = msg;
  el.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.add("hidden"), 2500);
}
