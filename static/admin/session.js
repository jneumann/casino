const STORAGE_KEY = "casino-backend.session";

export function saveSession(session) {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
}

export function clearSession() {
  window.localStorage.removeItem(STORAGE_KEY);
}

/** Returns the stored session, or null when absent, malformed, or expired. */
export function loadSession() {
  const raw = window.localStorage.getItem(STORAGE_KEY);
  if (!raw) {
    return null;
  }

  let session;
  try {
    session = JSON.parse(raw);
  } catch {
    clearSession();
    return null;
  }

  if (!session?.token) {
    clearSession();
    return null;
  }

  if (session.expiresAt && Date.parse(session.expiresAt) <= Date.now()) {
    clearSession();
    return null;
  }

  return session;
}
