import { saveSession, loadSession } from "/admin/session.js";

const form = document.getElementById("login-form");
const submit = document.getElementById("submit");
const errorBox = document.getElementById("error");

if (loadSession()) {
  window.location.replace("/status");
}

function showError(message) {
  errorBox.textContent = message;
  errorBox.hidden = false;
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorBox.hidden = true;

  const username = document.getElementById("username").value.trim();
  const password = document.getElementById("password").value;

  if (!username || !password) {
    showError("Enter your username and password.");
    return;
  }

  submit.disabled = true;
  submit.textContent = "Signing in\u2026";

  try {
    const response = await fetch("/api/admin/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password }),
    });

    const payload = await response.json().catch(() => null);

    if (!response.ok) {
      showError(payload?.message ?? "Sign in failed. Try again.");
      return;
    }

    saveSession({
      token: payload.token,
      expiresAt: payload.expires_at,
      username: payload.operator.username,
      role: payload.operator.role,
    });
    window.location.replace("/status");
  } catch {
    showError("Could not reach the server.");
  } finally {
    submit.disabled = false;
    submit.textContent = "Sign in";
  }
});
