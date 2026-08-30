import { loadSession, clearSession } from "/admin/session.js";

/** Redirects to the sign-in page when there is no usable session. */
export function requireSession() {
  const session = loadSession();
  if (!session) {
    window.location.replace("/admin");
    throw new Error("not signed in");
  }
  return session;
}

export function signOut() {
  clearSession();
  window.location.replace("/admin");
}

/**
 * Calls the admin API. Signs out on 401 so a revoked or expired operator
 * token cannot leave the page in a half-loaded state.
 */
export async function api(path, { method = "GET", body, token } = {}) {
  const headers = { Authorization: `Bearer ${token}` };
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
  }

  const response = await fetch(`/api/admin${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (response.status === 401) {
    signOut();
    throw new Error("Your session has ended. Please sign in again.");
  }

  if (response.status === 204) {
    return null;
  }

  const payload = await response.json().catch(() => null);

  if (!response.ok) {
    throw new Error(payload?.message ?? `Request failed (${response.status})`);
  }

  return payload;
}
