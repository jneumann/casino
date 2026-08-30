const BASE = "/api";

export class ApiError extends Error {
  constructor(message, { status = 0, code = "unknown" } = {}) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }

  /** True when the token is missing, expired, or rejected. */
  get isUnauthorized() {
    return this.status === 401;
  }
}

async function request(path, { method = "GET", body, token } = {}) {
  const headers = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (token) headers.Authorization = `Bearer ${token}`;

  let response;
  try {
    response = await fetch(`${BASE}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError("Could not reach the casino. Is the server running?", {
      code: "network",
    });
  }

  const payload = await response.json().catch(() => null);

  if (!response.ok) {
    throw new ApiError(payload?.message ?? `Request failed (${response.status})`, {
      status: response.status,
      code: payload?.error ?? "http_error",
    });
  }

  return payload;
}

export const api = {
  register: (username, password) =>
    request("/register", { method: "POST", body: { username, password } }),

  login: (username, password) =>
    request("/login", { method: "POST", body: { username, password } }),

  balance: (token) => request("/balance", { token }),

  me: (token) => request("/me", { token }),

  bank: (token) => request("/bank", { token }),

  borrow: (token, amount) =>
    request("/bank/borrow", { method: "POST", token, body: { amount } }),

  repay: (token) => request("/bank/repay", { method: "POST", token }),

  paytable: () => request("/slots/paytable"),

  spin: (token, bet) => request("/slots/spin", { method: "POST", token, body: { bet } }),
};
