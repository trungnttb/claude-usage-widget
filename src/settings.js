const { core } = window.__TAURI__;
const { stringsFor } = window.CUW_STRINGS;

const form = document.getElementById("form");
const savedNote = document.getElementById("saved");

/** Card toggles, in the order they appear on the widget. */
const CARD_KEYS = [
  "session",
  "forecast",
  "weekAll",
  "weekModel",
  "costToday",
  "chart",
  "models",
  "cacheHitRate",
  "topProject",
  "currentSession",
];

let settings = null;
let t = stringsFor("vi").settings;

function buildCardToggles() {
  const container = document.getElementById("cards");
  for (const key of CARD_KEYS) {
    const row = document.createElement("label");
    row.className = "check";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.name = `card.${key}`;
    const text = document.createElement("span");
    text.dataset.i18nCard = key;
    row.append(input, text);
    container.append(row);
  }
}

/**
 * Fills every element carrying a wording key.
 *
 * Labels live as keys in the markup rather than as text, so switching language
 * rewrites the window in place instead of rebuilding it.
 */
function applyWording() {
  document.title = `Claude Usage — ${t.title}`;
  for (const node of document.querySelectorAll("[data-i18n]")) {
    node.textContent = t[node.dataset.i18n] ?? node.dataset.i18n;
  }
  for (const node of document.querySelectorAll("[data-i18n-card]")) {
    node.textContent = t.cards[node.dataset.i18nCard] ?? node.dataset.i18nCard;
  }
}

function fill() {
  form.language.value = settings.language;
  form.trayMetric.value = settings.trayMetric;
  form.currencyCode.value = settings.currency.code;
  form.vndRate.value = settings.currency.vndRate;
  form.usageEnabled.checked = settings.usageEnabled;
  form.transcriptIntervalSeconds.value = settings.transcriptIntervalSeconds;
  form.usageIntervalSeconds.value = settings.usageIntervalSeconds;
  form.claudePath.value = settings.claudePath ?? "";
  form.notificationsEnabled.checked = settings.notificationsEnabled;
  form.sessionAlertPercent.value = settings.sessionAlertPercent;
  form.weekAlertPercent.value = settings.weekAlertPercent;
  form.alwaysOnTop.checked = settings.alwaysOnTop;
  form.skipTaskbar.checked = settings.skipTaskbar;
  form.autostart.checked = settings.autostart;
  form.opacity.value = settings.opacity;

  for (const key of CARD_KEYS) {
    form[`card.${key}`].checked = settings.cards[key];
  }
  applyConditionalFields();
}

/** Hides options that do not apply, rather than showing them inert. */
function applyConditionalFields() {
  document.getElementById("rate-field").hidden =
    form.currencyCode.value !== "VND";
  // set_skip_taskbar does nothing on macOS, so the option is not offered there.
  document.getElementById("skip-taskbar-field").hidden =
    navigator.userAgent.includes("Macintosh");
}

function collect() {
  return {
    ...settings,
    language: form.language.value,
    trayMetric: form.trayMetric.value,
    currency: {
      code: form.currencyCode.value,
      vndRate: Number(form.vndRate.value) || settings.currency.vndRate,
    },
    usageEnabled: form.usageEnabled.checked,
    transcriptIntervalSeconds: Number(form.transcriptIntervalSeconds.value),
    usageIntervalSeconds: Number(form.usageIntervalSeconds.value),
    claudePath: form.claudePath.value.trim() || null,
    notificationsEnabled: form.notificationsEnabled.checked,
    sessionAlertPercent: Number(form.sessionAlertPercent.value),
    weekAlertPercent: Number(form.weekAlertPercent.value),
    alwaysOnTop: form.alwaysOnTop.checked,
    skipTaskbar: form.skipTaskbar.checked,
    autostart: form.autostart.checked,
    opacity: Number(form.opacity.value),
    cards: Object.fromEntries(
      CARD_KEYS.map((key) => [key, form[`card.${key}`].checked]),
    ),
  };
}

function useSettings(next) {
  settings = next;
  t = stringsFor(settings.language).settings;
  applyWording();
}

let saveTimer = null;

/**
 * Saves on every change rather than behind a button. The backend clamps
 * anything out of range and returns what it stored, so the form always shows
 * what is actually in effect.
 */
function scheduleSave() {
  applyConditionalFields();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    useSettings(await core.invoke("save_settings", { next: collect() }));
    fill();
    savedNote.hidden = false;
    setTimeout(() => (savedNote.hidden = true), 1200);
  }, 250);
}

async function main() {
  buildCardToggles();
  useSettings(await core.invoke("get_settings"));
  fill();
  form.addEventListener("change", scheduleSave);
  form.addEventListener("input", scheduleSave);
}

main();
