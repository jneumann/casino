import { api, requireSession, signOut } from "/admin/api.js";

const session = requireSession();

const els = {
  error: document.getElementById("error"),
  notice: document.getElementById("notice"),
  signedInAs: document.getElementById("signed-in-as"),
  logout: document.getElementById("logout"),
  ownPasswordForm: document.getElementById("own-password-form"),
  currentPassword: document.getElementById("current-password"),
  newPassword: document.getElementById("new-password"),
  confirmPassword: document.getElementById("confirm-password"),
  addOperatorCard: document.getElementById("add-operator-card"),
  addOperatorForm: document.getElementById("add-operator-form"),
  newUsername: document.getElementById("new-username"),
  newOperatorPassword: document.getElementById("new-operator-password"),
  newRole: document.getElementById("new-role"),
  operators: document.getElementById("operators"),
  operatorsEmpty: document.getElementById("operators-empty"),
  operatorsSummary: document.getElementById("operators-summary"),
  players: document.getElementById("players"),
  playersEmpty: document.getElementById("players-empty"),
  playerSearch: document.getElementById("player-search"),
  playerSearchButton: document.getElementById("player-search-button"),
  adjustments: document.getElementById("adjustments"),
  adjustmentsEmpty: document.getElementById("adjustments-empty"),
  adjustmentsSummary: document.getElementById("adjustments-summary"),
  adjustDialog: document.getElementById("adjust-dialog"),
  adjustForm: document.getElementById("adjust-form"),
  adjustTarget: document.getElementById("adjust-target"),
  adjustDelta: document.getElementById("adjust-delta"),
  adjustReason: document.getElementById("adjust-reason"),
  adjustCancel: document.getElementById("adjust-cancel"),
  resetDialog: document.getElementById("reset-dialog"),
  resetForm: document.getElementById("reset-form"),
  resetTarget: document.getElementById("reset-target"),
  resetPassword: document.getElementById("reset-password"),
  resetCancel: document.getElementById("reset-cancel"),
};

/** Authoritative identity, fetched rather than trusted from localStorage. */
let me = null;
let resetTargetId = null;
let adjustTarget = null;

function showError(message) {
  els.error.textContent = message;
  els.error.hidden = false;
  els.notice.hidden = true;
}

function showNotice(message) {
  els.notice.textContent = message;
  els.notice.hidden = false;
  els.error.hidden = true;
}

function clearMessages() {
  els.error.hidden = true;
  els.notice.hidden = true;
}

function formatDate(value) {
  return value ? new Date(value).toLocaleString() : "never";
}

function badge(text, variant) {
  const span = document.createElement("span");
  span.className = `badge badge--${variant}`;
  span.textContent = text;
  return span;
}

function cell(row, content, className) {
  const td = document.createElement("td");
  if (className) td.className = className;
  if (content instanceof Node) {
    td.append(content);
  } else {
    td.textContent = content;
  }
  row.append(td);
  return td;
}

function actionButton(label, { variant = "", onClick }) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `button button--small ${variant}`.trim();
  button.textContent = label;
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await onClick();
    } catch (err) {
      showError(err.message);
    } finally {
      button.disabled = false;
    }
  });
  return button;
}

function renderOperators(operators) {
  const isAdmin = me?.role === "admin";
  els.operatorsEmpty.hidden = operators.length > 0;

  els.operators.replaceChildren(
    ...operators.map((operator) => {
      const row = document.createElement("tr");
      const isSelf = operator.id === me?.id;

      cell(
        row,
        isSelf ? `${operator.username} (you)` : operator.username,
        "who",
      );
      cell(row, badge(operator.role, operator.role));
      cell(
        row,
        operator.is_active
          ? badge("active", "active")
          : badge("deactivated", "inactive"),
      );
      cell(row, formatDate(operator.last_login_at), operator.last_login_at ? "" : "stale");

      const actions = document.createElement("div");
      actions.className = "row-actions";

      if (isAdmin && !isSelf) {
        actions.append(
          actionButton("Reset password", {
            onClick: () => openResetDialog(operator),
          }),
          actionButton(operator.role === "admin" ? "Make viewer" : "Make admin", {
            onClick: () =>
              updateOperator(operator.id, {
                role: operator.role === "admin" ? "viewer" : "admin",
              }),
          }),
          actionButton(operator.is_active ? "Deactivate" : "Reactivate", {
            onClick: () =>
              updateOperator(operator.id, { is_active: !operator.is_active }),
          }),
          actionButton("Delete", {
            variant: "button--danger",
            onClick: () => deleteOperator(operator),
          }),
        );
      }

      cell(row, actions);
      return row;
    }),
  );

  const admins = operators.filter(
    (operator) => operator.role === "admin" && operator.is_active,
  ).length;
  els.operatorsSummary.textContent = `${operators.length} operator${
    operators.length === 1 ? "" : "s"
  }, ${admins} active admin${admins === 1 ? "" : "s"}`;
}

function renderPlayers(players) {
  const isAdmin = me?.role === "admin";
  els.playersEmpty.hidden = players.length > 0;

  els.players.replaceChildren(
    ...players.map((player) => {
      const row = document.createElement("tr");

      cell(row, player.username, "who");

      const balance = document.createElement("div");
      balance.className = "balance-cell";
      const amount = document.createElement("span");
      amount.textContent = player.balance.toLocaleString();
      balance.append(amount);
      if (isAdmin) {
        balance.append(
          actionButton("Adjust", {
            onClick: () => openAdjustDialog(player),
          }),
        );
      }
      cell(row, balance, "numeric");

      cell(
        row,
        player.is_banned ? badge("banned", "banned") : badge("active", "active"),
      );
      cell(row, formatDate(player.last_login_at), player.last_login_at ? "" : "stale");

      const actions = document.createElement("div");
      actions.className = "row-actions";

      if (isAdmin) {
        actions.append(
          actionButton(player.is_banned ? "Unban" : "Ban", {
            variant: player.is_banned ? "" : "button--danger",
            onClick: () => setPlayerBanned(player),
          }),
        );
      }

      cell(row, actions);
      return row;
    }),
  );
}

async function loadOperators() {
  renderOperators(await api("/operators", { token: session.token }));
}

async function loadPlayers() {
  const term = els.playerSearch.value.trim();
  const query = term ? `?search=${encodeURIComponent(term)}` : "";
  renderPlayers(await api(`/players${query}`, { token: session.token }));
}

function formatSigned(value) {
  const formatted = Math.abs(value).toLocaleString();
  if (value > 0) return `+${formatted}`;
  if (value < 0) return `\u2212${formatted}`;
  return formatted;
}

function renderAdjustments(adjustments) {
  if (!els.adjustments || !els.adjustmentsEmpty) return;
  els.adjustmentsEmpty.hidden = adjustments.length > 0;

  els.adjustments.replaceChildren(
    ...adjustments.map((adjustment) => {
      const row = document.createElement("tr");
      const amount = document.createElement("span");
      amount.className =
        adjustment.delta >= 0 ? "amount amount--credit" : "amount amount--debit";
      amount.textContent = formatSigned(adjustment.delta);

      cell(row, formatDate(adjustment.created_at));
      cell(row, adjustment.player_username, "who");
      cell(row, amount, "numeric");
      cell(row, adjustment.balance_after.toLocaleString(), "numeric");
      cell(row, adjustment.operator_username, "who");
      cell(row, adjustment.reason);

      return row;
    }),
  );

  els.adjustmentsSummary.textContent =
    adjustments.length === 0
      ? "No recent changes"
      : `${adjustments.length} recent change${
          adjustments.length === 1 ? "" : "s"
        }`;
}

async function loadAdjustments() {
  renderAdjustments(await api("/adjustments", { token: session.token }));
}

async function updateOperator(id, changes) {
  await api(`/operators/${id}`, {
    method: "PATCH",
    body: changes,
    token: session.token,
  });
  showNotice("Operator updated.");
  await loadOperators();
}

async function deleteOperator(operator) {
  const confirmed = window.confirm(
    `Delete operator "${operator.username}"? This cannot be undone.`,
  );
  if (!confirmed) return;

  await api(`/operators/${operator.id}`, {
    method: "DELETE",
    token: session.token,
  });
  showNotice(`Deleted ${operator.username}.`);
  await loadOperators();
}

async function setPlayerBanned(player) {
  if (!player.is_banned) {
    const confirmed = window.confirm(
      `Ban "${player.username}"? They will be signed out and unable to play.`,
    );
    if (!confirmed) return;
  }

  await api(`/players/${player.id}`, {
    method: "PATCH",
    body: { is_banned: !player.is_banned },
    token: session.token,
  });
  showNotice(
    `${player.username} is now ${player.is_banned ? "active" : "banned"}.`,
  );
  await loadPlayers();
}

function openAdjustDialog(player) {
  adjustTarget = player;
  els.adjustTarget.textContent = `${player.username} currently has ${player.balance.toLocaleString()} coins.`;
  els.adjustDelta.value = "";
  els.adjustReason.value = "";
  els.adjustDelta.setCustomValidity("");
  els.adjustReason.setCustomValidity("");
  els.adjustDialog.showModal();
}

els.adjustCancel?.addEventListener("click", () => els.adjustDialog.close());

els.adjustForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!adjustTarget) return;

  const delta = Number.parseInt(els.adjustDelta.value, 10);
  const reason = els.adjustReason.value.trim();

  if (!Number.isInteger(delta) || delta === 0) {
    els.adjustDelta.setCustomValidity("Enter a non-zero whole number of coins.");
    els.adjustDelta.reportValidity();
    return;
  }
  els.adjustDelta.setCustomValidity("");
  if (!reason) {
    els.adjustReason.setCustomValidity("A reason is required for every adjustment.");
    els.adjustReason.reportValidity();
    return;
  }
  els.adjustReason.setCustomValidity("");

  try {
    const result = await api(`/players/${adjustTarget.id}/balance`, {
      method: "POST",
      body: { delta, reason },
      token: session.token,
    });
    els.adjustDialog.close();
    showNotice(
      `${result.player.username} now has ${result.player.balance.toLocaleString()} coins.`,
    );
    await Promise.all([loadPlayers(), loadAdjustments()]);
  } catch (err) {
    showError(err.message);
    els.adjustDialog.close();
  }
});

function openResetDialog(operator) {
  resetTargetId = operator.id;
  els.resetTarget.textContent = `New password for ${operator.username}.`;
  els.resetPassword.value = "";
  els.resetDialog.showModal();
}

els.resetCancel.addEventListener("click", () => els.resetDialog.close());

els.resetForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const newPassword = els.resetPassword.value;

  try {
    await api(`/operators/${resetTargetId}/password`, {
      method: "PUT",
      body: { new_password: newPassword },
      token: session.token,
    });
    els.resetDialog.close();
    showNotice("Password reset. Their other sessions are signed out.");
    await loadOperators();
  } catch (err) {
    showError(err.message);
    els.resetDialog.close();
  }
});

els.addOperatorForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  clearMessages();

  try {
    const created = await api("/operators", {
      method: "POST",
      body: {
        username: els.newUsername.value.trim(),
        password: els.newOperatorPassword.value,
        role: els.newRole.value,
      },
      token: session.token,
    });

    els.addOperatorForm.reset();
    showNotice(`Created ${created.username} as ${created.role}.`);
    await loadOperators();
  } catch (err) {
    showError(err.message);
  }
});

els.ownPasswordForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  clearMessages();

  if (els.newPassword.value !== els.confirmPassword.value) {
    showError("The new passwords do not match.");
    return;
  }

  try {
    await api("/me/password", {
      method: "PUT",
      body: {
        current_password: els.currentPassword.value,
        new_password: els.newPassword.value,
      },
      token: session.token,
    });

    els.ownPasswordForm.reset();
    showNotice(
      "Password updated. This session stays signed in; others were ended.",
    );
  } catch (err) {
    showError(err.message);
  }
});

els.playerSearchButton.addEventListener("click", () => {
  loadPlayers().catch((err) => showError(err.message));
});

els.playerSearch.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    loadPlayers().catch((err) => showError(err.message));
  }
});

els.logout.addEventListener("click", signOut);

async function start() {
  try {
    me = await api("/me", { token: session.token });
    els.signedInAs.textContent = `Signed in as ${me.username} (${me.role})`;
    els.addOperatorCard.hidden = me.role !== "admin";

    await Promise.all([loadOperators(), loadPlayers()]);
    try {
      await loadAdjustments();
    } catch (err) {
      showError(err.message);
    }

    if (me.role !== "admin") {
      showNotice(
        "You have viewer access, so account changes are limited to your own password.",
      );
    }
  } catch (err) {
    showError(err.message);
  }
}

start();
