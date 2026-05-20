// =============================================================================
// Kanban WebUI — Vanilla JS Application
// =============================================================================

const PRIORITY_ORDER = ["urgent", "high", "medium", "low", "backlog"];
const PRIORITY_CHARS = { urgent: "!", high: "!", medium: "·", low: ".", backlog: " " };
const PRIORITY_COLORS = {
  urgent: "var(--urgent)", high: "var(--high)", medium: "var(--medium)",
  low: "var(--low)", backlog: "var(--backlog)",
};
const COLUMNS = ["backlog", "todo", "in_progress", "review", "done"];

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------
const state = {
  focus: "cards",           // 'columns' | 'cards' | 'detail'
  boardName: "Loading",
  columns: [],              // [{name, cards: [{...}]}]
  allCards: [],             // flat list
  currentColumnIdx: 0,
  cardSelection: 0,
  detailCard: null,
  mode: "normal",           // 'normal' | 'moving' | 'searching' | 'editing'
  searchQuery: "",
  message: null,
  messageTime: 0,
  searchingResults: [],
  sse: null,
  sseReconnectTimer: null,
  sseReconnectDelay: 1000,
};

// ---------------------------------------------------------------------------
// API helpers
// ---------------------------------------------------------------------------
const API = {
  async get(path) {
    const res = await fetch(path);
    if (!res.ok) throw new Error(`API ${res.status}`);
    return res.json();
  },
  async post(path, body) {
    const res = await fetch(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`API ${res.status}: ${await res.text()}`);
    return res.json();
  },
  async patch(path, body) {
    const res = await fetch(path, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`API ${res.status}: ${await res.text()}`);
    return res.json();
  },
  async del(path) {
    const res = await fetch(path, { method: "DELETE" });
    if (!res.ok) throw new Error(`API ${res.status}: ${await res.text()}`);
    return res.json();
  },
};

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------
async function loadBoard() {
  try {
    const data = await API.get("/api/cards");
    state.boardName = data.name || "Board";
    state.columns = data.columns;
    state.allCards = state.columns.flatMap((c) => c.cards);
    state.currentColumnIdx = Math.max(0, Math.min(state.currentColumnIdx, state.columns.length - 1));
    normalizeFocusForCurrentCards();
    render();
  } catch (e) {
    showMessage("Error loading board: " + e.message);
  }
}

async function refreshCurrentColumn() {
  await loadBoard();
}

async function refreshAll() {
  await loadBoard();
}

async function searchCards(query) {
  try {
    const cards = await API.get(`/api/cards/search?q=${encodeURIComponent(query)}`);
    state.searchingResults = cards;
    state.mode = "searchingResult";
    state.cardSelection = 0;
    state.detailCard = null;
    normalizeFocusForCurrentCards();
    render();
  } catch (e) {
    showMessage("Search failed: " + e.message);
  }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------
async function moveCard(cardId, targetColumn) {
  try {
    await API.post(`/api/cards/${cardId}/move`, { column: targetColumn });
    await refreshAll();
    showMessage(`Moved card to ${targetColumn}`);
  } catch (e) {
    showMessage(`Error: ${e.message}`);
  }
}

async function deleteCard(cardId, title) {
  try {
    await API.del(`/api/cards/${cardId}`);
    state.detailCard = null;
    state.cardSelection = 0;
    await refreshAll();
    showMessage(`Deleted '${title}'`);
  } catch (e) {
    showMessage(`Error: ${e.message}`);
  }
}

async function updateCard(cardId, updates) {
  try {
    await API.patch(`/api/cards/${cardId}`, updates);
    state.mode = "normal";
    state.detailCard = null;
    await refreshAll();
    showMessage("Card updated");
  } catch (e) {
    showMessage(`Error: ${e.message}`);
  }
}

// ---------------------------------------------------------------------------
// DOM rendering
// ---------------------------------------------------------------------------
function render() {
  renderColumnsPanel();
  renderCardsPanel();
  renderDetailPanel();
  renderTopBar();
  renderBottomBar();
}

function renderTopBar() {
  const total = state.allCards.length;
  const searchInput = document.getElementById("search-input");
  const cardCount = document.getElementById("card-count");

  document.getElementById("project-name").textContent = `📋 ${state.boardName}`;
  cardCount.textContent = `Cards: ${total}`;
  searchInput.classList.toggle("hidden", state.mode !== "searching");
}

function renderBottomBar() {
  const focusMap = { columns: "Columns", cards: "Cards", detail: "Detail" };
  const focusText = focusMap[state.focus] || "Cards";
  const el = document.getElementById("focus-indicator");
  el.textContent = `Focus: ${focusText}`;

  // Append message if present
  const bar = document.getElementById("bottom-bar");
  const existingMsg = bar.querySelector(".bar-message");
  if (existingMsg) existingMsg.remove();

  if (state.message) {
    const msg = document.createElement("span");
    msg.className = "bar-message";
    msg.textContent = ` | ${state.message}`;
    bar.appendChild(msg);
  }
}

function getCurrentCards() {
  if (state.mode === "searchingResult") {
    return state.searchingResults;
  }
  const col = state.columns[state.currentColumnIdx];
  return col ? col.cards : [];
}

function normalizeFocusForCurrentCards() {
  const cards = getCurrentCards();
  if (cards.length === 0) {
    state.cardSelection = 0;
    state.detailCard = null;
    if (state.focus === "cards" || state.focus === "detail") {
      state.focus = "columns";
    }
    return;
  }
  state.cardSelection = Math.min(state.cardSelection, cards.length - 1);
}

function exitSearchMode() {
  state.mode = "normal";
  state.searchQuery = "";
  state.searchingResults = [];
  state.detailCard = null;
  normalizeFocusForCurrentCards();
  render();
}

function renderColumnsPanel() {
  const container = document.getElementById("columns-list");
  container.innerHTML = "";

  state.columns.forEach((col, i) => {
    const div = document.createElement("div");
    div.className = `column-item${i === state.currentColumnIdx ? " active" : ""}`;
    div.textContent = `${col.name} (${col.cards.length})`;
    div.dataset.idx = i;

    div.addEventListener("click", () => {
      state.currentColumnIdx = i;
      state.cardSelection = 0;
      state.detailCard = null;
      state.mode = "normal";
      normalizeFocusForCurrentCards();
      render();
    });

    container.appendChild(div);
  });
}

function renderCardsPanel() {
  const cards = getCurrentCards();
  const container = document.getElementById("cards-list");
  container.innerHTML = "";

  // Update panel title
  const title = state.mode === "searchingResult"
    ? "Search Results"
    : state.columns[state.currentColumnIdx]?.name || "?";
  document.getElementById("cards-title").textContent = title;

  if (cards.length === 0) {
    container.innerHTML = '<div class="card-item" style="color:var(--text-dim);cursor:default">(empty)</div>';
    return;
  }

  cards.forEach((card, i) => {
    const div = document.createElement("div");
    div.className = `card-item${i === state.cardSelection ? " selected" : ""} ${card.priority}`;
    div.draggable = true;
    div.dataset.idx = i;
    div.dataset.id = card.id;

    const ch = PRIORITY_CHARS[card.priority] || " ";
    const color = PRIORITY_COLORS[card.priority] || "var(--text)";
    const title = card.title.length > 30 ? card.title.slice(0, 27) + "…" : card.title;
    const shortId = card.id.slice(0, 8);

    div.textContent = `${ch} ${shortId} ${title}`;
    div.style.borderLeftColor = i === state.cardSelection ? "" : color;

    // Click to select
    div.addEventListener("click", () => {
      state.cardSelection = i;
      state.detailCard = card;
      state.focus = "cards";
      render();
    });

    // Drag start
    div.addEventListener("dragstart", (e) => {
      e.dataTransfer.setData("text/plain", card.id);
      e.dataTransfer.effectAllowed = "move";
      e.stopPropagation();
    });

    container.appendChild(div);
  });
}

function renderDetailPanel() {
  const content = document.getElementById("detail-content");

  if (state.mode === "editing" && state.detailCard) {
    renderEditMode(content, state.detailCard);
  } else if (state.detailCard) {
    renderDetailView(content, state.detailCard);
  } else {
    content.innerHTML = '<p class="empty-hint">No card selected</p>';
  }
}

function renderDetailView(container, card) {
  const labelsStr = card.labels.length > 0 ? card.labels.join(", ") : "none";
  const idShort = card.id.slice(0, 12);
  const desc = escapeHtml(card.description) || '<span style="color:var(--text-dim)">(no description)</span>';
  const truncated = card.description.length > 500;

  container.innerHTML = `
    <div class="detail-title">${escapeHtml(card.title)}</div>
    <div class="detail-meta">
      <div class="detail-meta-line">ID: ${idShort}</div>
      <div class="detail-meta-line">Priority: ${card.priority}</div>
      <div class="detail-meta-line">Labels: ${escapeHtml(labelsStr)}</div>
    </div>
    <div style="border-top:1px solid var(--border);margin-top:8px;padding-top:8px" class="detail-body${truncated ? ' truncated' : ''}">
      ${desc}
    </div>
  `;
}

function renderEditMode(container, card) {
  const labelsStr = card.labels.join(", ");
  const priorityOptions = PRIORITY_ORDER.map(
    (p) => `<option value="${p}"${p === card.priority ? " selected" : ""}>${p}</option>`
  ).join("");

  container.innerHTML = `
    <input type="text" class="edit-title-input" id="edit-title" value="${escapeAttr(card.title)}" placeholder="Card title">
    <div class="edit-row">
      <select class="edit-priority-select" id="edit-priority">
        ${priorityOptions}
      </select>
    </div>
    <input type="text" class="edit-labels-input" id="edit-labels" value="${escapeAttr(labelsStr)}" placeholder="labels,comma,separated">
    <textarea class="edit-description" id="edit-description" placeholder="Description (markdown)">${escapeHtml(card.description)}</textarea>
    <div style="display:flex;gap:8px;margin-top:8px">
      <button class="edit-save" id="edit-save">Save</button>
      <button class="edit-cancel" id="edit-cancel">Cancel</button>
    </div>
  `;

  // Focus title
  const titleInput = document.getElementById("edit-title");
  titleInput.focus();
  titleInput.setSelectionRange(titleInput.value.length, titleInput.value.length);

  // Save
  document.getElementById("edit-save").addEventListener("click", async () => {
    const title = document.getElementById("edit-title").value.trim();
    if (!title) {
      showMessage("Title is required");
      return;
    }
    const description = document.getElementById("edit-description").value;
    const priority = document.getElementById("edit-priority").value;
    const labels = document
      .getElementById("edit-labels")
      .value.split(",")
      .map((s) => s.trim())
      .filter(Boolean);

    await updateCard(card.id, { title, description, priority, labels });
  });

  // Cancel
  document.getElementById("edit-cancel").addEventListener("click", () => {
    state.mode = "normal";
    render();
  });

  // Enter key in title saves
  titleInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      document.getElementById("edit-save").click();
    }
  });
}

// ---------------------------------------------------------------------------
// Modals
// ---------------------------------------------------------------------------
function showModal(id) {
  document.getElementById(id).classList.remove("hidden");
}
function hideModal(id) {
  document.getElementById(id).classList.add("hidden");
}
function hideAllModals() {
  document.querySelectorAll(".modal").forEach((m) => m.classList.add("hidden"));
}

function openMoveModal() {
  const card = getCurrentCards()[state.cardSelection];
  if (!card) return;

  const container = document.getElementById("move-columns");
  container.innerHTML = "";

  COLUMNS.forEach((colName) => {
    const col = state.columns.find((c) => c.name === colName);
    const count = col ? col.cards.length : 0;
    const div = document.createElement("div");
    div.className = "move-col";
    div.innerHTML = `<span>${colName}</span><span class="key">[${colName[0].toUpperCase()}]</span><span style="color:var(--text-dim);margin-left:8px">(${count})</span>`;
    div.addEventListener("click", () => {
      hideModal("move-modal");
      moveCard(card.id, colName);
    });
    container.appendChild(div);
  });

  showModal("move-modal");
}

function openDeleteModal() {
  const card = getCurrentCards()[state.cardSelection];
  if (!card) return;

  document.getElementById("delete-card-title").textContent = `'${card.title}'`;
  document.getElementById("delete-confirm-buttons").innerHTML = `
    <div class="delete-buttons">
      <button class="btn-yes" id="delete-yes">Yes, Delete</button>
      <button class="btn-no" id="delete-no">Cancel</button>
    </div>
  `;

  document.getElementById("delete-yes").addEventListener("click", () => {
    hideModal("delete-modal");
    deleteCard(card.id, card.title);
  });

  document.getElementById("delete-no").addEventListener("click", () => {
    hideModal("delete-modal");
  });

  showModal("delete-modal");
}

function openSearchMode() {
  state.mode = "searching";
  state.searchQuery = "";
  const searchField = document.getElementById("search-field");
  searchField.value = "";
  searchField.focus();
  renderTopBar();
}

// ---------------------------------------------------------------------------
// Message display
// ---------------------------------------------------------------------------
function showMessage(msg) {
  state.message = msg;
  state.messageTime = Date.now();
  renderBottomBar();
  setTimeout(() => {
    if (state.message === msg) {
      state.message = null;
      renderBottomBar();
    }
  }, 3000);
}

// ---------------------------------------------------------------------------
// SSE connection
// ---------------------------------------------------------------------------
function setSSEStatus(status) {
  document.body.dataset.sse = status;
}

function connectSSE() {
  if (state.sse && state.sse.readyState !== EventSource.CLOSED) {
    return;
  }
  if (state.sseReconnectTimer) {
    clearTimeout(state.sseReconnectTimer);
    state.sseReconnectTimer = null;
  }

  try {
    setSSEStatus("connecting");
    const evtSource = new EventSource("/api/events");
    state.sse = evtSource;

    evtSource.onopen = () => {
      state.sseReconnectDelay = 1000;
      setSSEStatus("connected");
    };

    evtSource.addEventListener("card_created", () => {
      refreshCurrentColumn();
    });

    evtSource.addEventListener("card_moved", () => {
      refreshAll();
    });

    evtSource.addEventListener("card_updated", () => {
      refreshCurrentColumn();
    });

    evtSource.addEventListener("card_deleted", () => {
      refreshCurrentColumn();
    });

    evtSource.onerror = () => {
      console.log("SSE connection lost, reconnecting...");
      setSSEStatus("reconnecting");
      evtSource.close();
      if (state.sse === evtSource) {
        state.sse = null;
      }

      const delay = state.sseReconnectDelay;
      state.sseReconnectDelay = Math.min(state.sseReconnectDelay * 2, 30000);
      state.sseReconnectTimer = setTimeout(() => {
        state.sseReconnectTimer = null;
        connectSSE();
      }, delay);
    };
  } catch (e) {
    setSSEStatus("unavailable");
    console.log("SSE not available:", e);
  }
}

// ---------------------------------------------------------------------------
// Keyboard handling
// ---------------------------------------------------------------------------
document.addEventListener("keydown", (e) => {
  // Don't capture keys when typing in inputs
  if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA" || e.target.tagName === "SELECT") {
    if (e.key === "Escape") {
      e.target.blur();
      e.preventDefault();
    }
    // In search input, handle backspace/enter
    if (e.target.id === "search-field") {
      if (e.key === "Enter") {
        const query = e.target.value.trim();
        if (query) {
          searchCards(query);
        } else {
          exitSearchMode();
        }
      } else if (e.key === "Escape") {
        exitSearchMode();
      }
      return;
    }
    // In edit mode, Escape cancels
    if (state.mode === "editing" && e.key === "Escape") {
      state.mode = "normal";
      render();
    }
    return;
  }

  // Move mode
  if (state.mode === "moving" || document.getElementById("move-modal").classList.contains("hidden") === false) {
    const keyMap = { b: "backlog", t: "todo", i: "in_progress", r: "review", d: "done" };
    if (keyMap[e.key.toLowerCase()]) {
      const card = getCurrentCards()[state.cardSelection];
      if (card) moveCard(card.id, keyMap[e.key.toLowerCase()]);
      return;
    }
    if (e.key === "Escape" || e.key === "q") {
      hideModal("move-modal");
      return;
    }
    return;
  }

  // Delete modal
  if (document.getElementById("delete-modal").classList.contains("hidden") === false) {
    if (e.key === "y" || e.key === "Y") {
      const card = getCurrentCards()[state.cardSelection];
      if (card) {
        hideModal("delete-modal");
        deleteCard(card.id, card.title);
      }
    }
    if (e.key === "n" || e.key === "N" || e.key === "Escape") {
      hideModal("delete-modal");
    }
    return;
  }

  // Normal mode
  switch (e.key) {
    case "q":
      break; // Close tab
    case "ArrowRight":
    case "l":
      focusNext(true);
      break;
    case "ArrowLeft":
    case "h":
      focusNext(false);
      break;
    case "ArrowUp":
    case "k":
      if (state.focus === "columns") {
        columnNext(true);
      } else {
        cardNav(false);
      }
      break;
    case "ArrowDown":
    case "j":
      if (state.focus === "columns") {
        columnNext(false);
      } else {
        cardNav(true);
      }
      break;
    case "Enter":
      handleEnter();
      break;
    case "Escape":
      handleEscape();
      break;
    case "m":
      openMoveModal();
      break;
    case "e":
      openEditMode();
      break;
    case "D":
      openDeleteModal();
      break;
    case "/":
      openSearchMode();
      break;
  }
});

function focusNext(forward) {
  const panels = ["columns", "cards", "detail"];
  const idx = panels.indexOf(state.focus);
  const newIdx = forward
    ? (idx + 1) % panels.length
    : (idx - 1 + panels.length) % panels.length;
  state.focus = panels[newIdx];
  normalizeFocusForCurrentCards();
  if (newIdx === 2) {
    // Focus detail — select current card
    const cards = getCurrentCards();
    if (cards.length > 0 && state.cardSelection < cards.length) {
      state.detailCard = cards[state.cardSelection];
    }
  }
  if (newIdx !== 2) {
    state.detailCard = null;
  }
  render();
}

function columnNext(forward) {
  const cols = state.columns.length;
  if (cols === 0) return;
  state.currentColumnIdx = forward
    ? (state.currentColumnIdx + 1) % cols
    : state.currentColumnIdx === 0
      ? cols - 1
      : state.currentColumnIdx - 1;
  state.cardSelection = 0;
  state.detailCard = null;
  normalizeFocusForCurrentCards();
  render();
}

function cardNav(forward) {
  const cards = getCurrentCards();
  if (cards.length === 0) {
    state.focus = "columns";
    state.detailCard = null;
    render();
    return;
  }
  const newIdx = forward
    ? Math.min(state.cardSelection + 1, cards.length - 1)
    : Math.max(state.cardSelection - 1, 0);
  state.cardSelection = newIdx;
  state.detailCard = cards[newIdx];
  render();
}

function handleEnter() {
  if (state.focus === "columns") {
    // Select first card in that column
    const col = state.columns[state.currentColumnIdx];
    if (col && col.cards.length > 0) {
      state.focus = "cards";
      state.cardSelection = 0;
      state.detailCard = col.cards[0];
      render();
    }
  } else if (state.focus === "cards") {
    const cards = getCurrentCards();
    if (cards.length > 0 && state.cardSelection < cards.length) {
      state.detailCard = cards[state.cardSelection];
      state.focus = "detail";
      render();
    }
  } else if (state.focus === "detail") {
    state.detailCard = null;
    state.focus = "cards";
    render();
  }
}

function handleEscape() {
  if (state.mode === "searching" || state.mode === "searchingResult") {
    exitSearchMode();
    return;
  }
  state.detailCard = null;
  hideAllModals();
  state.mode = "normal";
  normalizeFocusForCurrentCards();
  render();
}

function openEditMode() {
  const card = getCurrentCards()[state.cardSelection];
  if (!card) return;
  state.detailCard = card;
  state.mode = "editing";
  state.focus = "detail";
  render();
}

// ---------------------------------------------------------------------------
// Drag and drop — cards panel as drop target
// ---------------------------------------------------------------------------
const cardsPanel = document.getElementById("cards-panel");

cardsPanel.addEventListener("dragover", (e) => {
  e.preventDefault();
  e.dataTransfer.dropEffect = "move";
  cardsPanel.classList.add("drop-target");
});

cardsPanel.addEventListener("dragleave", () => {
  cardsPanel.classList.remove("drop-target");
});

cardsPanel.addEventListener("drop", (e) => {
  e.preventDefault();
  cardsPanel.classList.remove("drop-target");
  const cardId = e.dataTransfer.getData("text/plain");
  if (!cardId) return;

  const col = state.columns[state.currentColumnIdx];
  if (col) {
    moveCard(cardId, col.name);
  }
});

// ---------------------------------------------------------------------------
// Click outside modal to close
// ---------------------------------------------------------------------------
document.querySelectorAll(".modal").forEach((modal) => {
  modal.addEventListener("click", (e) => {
    if (e.target === modal) {
      hideAllModals();
    }
  });
});

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------
function escapeHtml(str) {
  if (!str) return "";
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

function escapeAttr(str) {
  if (!str) return "";
  return str.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/'/g, "&#39;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
async function init() {
  try {
    await loadBoard();
    connectSSE();
    showMessage("Ready");
  } catch (e) {
    document.body.innerHTML = `<div style="padding:20px;color:var(--urgent)">Error: ${escapeHtml(e.message)}</div>`;
  }
}

init();
