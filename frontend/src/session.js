// Deliberately distinct from the operator dashboard's key, so signing into
// /admin in the same browser does not clobber a player's session.
const STORAGE_KEY = "casino.player.session";

/** Turns a login/register response into the shape we persist. */
export function sessionFromAuth(payload) {
  return {
    token: payload.token,
    expiresAt: payload.expires_at,
    username: payload.user.username,
    balance: payload.user.balance,
    loan: payload.user.loan ?? null,
  };
}

export function saveSession(session) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
  } catch {
    // Private browsing can refuse writes; the session just won't outlive the tab.
  }
}

export function clearSession() {
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    /* nothing to clean up */
  }
}

/** Returns the stored session, or null when absent, malformed, or expired. */
export function loadSession() {
  let raw = null;
  try {
    raw = window.localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }

  if (!raw) return null;

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
