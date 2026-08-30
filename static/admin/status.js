import { loadSession, clearSession } from "/admin/session.js";

const REFRESH_MS = 15_000;

const session = loadSession();
if (!session) {
  window.location.replace("/admin");
}

const els = {
  error: document.getElementById("error"),
  overall: document.getElementById("overall"),
  signedInAs: document.getElementById("signed-in-as"),
  uptime: document.getElementById("uptime"),
  startedAt: document.getElementById("started-at"),
  version: document.getElementById("version"),
  environment: document.getElementById("environment"),
  dbLatency: document.getElementById("db-latency"),
  dbBackend: document.getElementById("db-backend"),
  userCount: document.getElementById("user-count"),
  checks: document.getElementById("checks"),
  generatedAt: document.getElementById("generated-at"),
  refresh: document.getElementById("refresh"),
  logout: document.getElementById("logout"),
};

els.signedInAs.textContent = `Signed in as ${session?.username ?? "unknown"}`;

function signOut() {
  clearSession();
  window.location.replace("/admin");
}

function formatUptime(totalSeconds) {
  const seconds = Math.max(0, Number(totalSeconds) || 0);
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}

function renderChecks(checks) {
  els.checks.replaceChildren(
    ...checks.map((check) => {
      const item = document.createElement("li");
      item.className = "check";

      const dot = document.createElement("span");
      dot.className = `dot dot--${check.health}`;

      const name = document.createElement("span");
      name.className = "check__name";
      name.textContent = check.name;

      const detail = document.createElement("span");
      detail.className = "check__detail";
      detail.textContent = check.detail;

      item.append(dot, name, detail);
      return item;
    }),
  );
}

function render(status) {
  els.overall.textContent = status.overall;
  els.overall.className = `badge badge--${status.overall}`;

  els.uptime.textContent = formatUptime(status.service.uptime_seconds);
  els.startedAt.textContent = `since ${new Date(
    status.service.started_at,
  ).toLocaleString()}`;

  els.version.textContent = `v${status.service.version}`;
  els.environment.textContent = status.service.environment;

  els.dbLatency.textContent = status.database.reachable
    ? `${status.database.latency_ms?.toFixed(2) ?? "?"} ms`
    : "offline";
  els.dbBackend.textContent = status.database.reachable
    ? status.database.backend
    : (status.database.error ?? "unreachable");

  els.userCount.textContent = status.database.users ?? "\u2014";
  els.generatedAt.textContent = new Date(
    status.generated_at,
  ).toLocaleTimeString();

  renderChecks(status.checks);
}

async function refresh() {
  els.refresh.disabled = true;

  try {
    const response = await fetch("/api/status", {
      headers: { Authorization: `Bearer ${session.token}` },
    });

    if (response.status === 401) {
      signOut();
      return;
    }

    if (!response.ok) {
      const payload = await response.json().catch(() => null);
      throw new Error(payload?.message ?? `Request failed (${response.status})`);
    }

    render(await response.json());
    els.error.hidden = true;
  } catch (err) {
    els.error.textContent = err.message ?? "Could not load status.";
    els.error.hidden = false;
    els.overall.textContent = "unknown";
    els.overall.className = "badge badge--muted";
  } finally {
    els.refresh.disabled = false;
  }
}

els.refresh.addEventListener("click", refresh);
els.logout.addEventListener("click", signOut);

refresh();
setInterval(refresh, REFRESH_MS);
