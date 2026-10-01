const { event, core, window: tauriWindow } = window.__TAURI__;
const { stringsFor, formatClockFor } = window.CUW_STRINGS;

/** Latest snapshot from the backend. The frontend computes nothing else. */
let snapshot = null;
let settings = null;
/** Wording for the configured language, refreshed whenever settings change. */
let t = stringsFor("vi");

const root = document.getElementById("widget");

// -- formatting ------------------------------------------------------------

/**
 * The largest two units that still carry information: "4 ngày 15h", "1h52m",
 * "12m". Weekly windows run to over a hundred hours, which nobody reads as a
 * length of time, so days take over past a day.
 */
function formatDuration(seconds) {
  if (seconds <= 0) return "0s";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  // Vietnamese spaces the unit as a word, English abbreviates it.
  const withDays =
    settings.language === "vi" ? `${days} ngày ${hours}h` : `${days}d ${hours}h`;
  if (days > 0) return withDays;
  if (hours > 0) return `${hours}h${String(minutes).padStart(2, "0")}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

function formatClock(unixSeconds) {
  return formatClockFor(settings.language, unixSeconds);
}

/** Below 50% reads as fine, 80% and over as the point to act. */
function level(percent) {
  if (percent >= 80) return "danger";
  if (percent >= 50) return "warn";
  return "ok";
}

function nowSeconds() {
  return Math.floor(Date.now() / 1000);
}

// -- building blocks -------------------------------------------------------

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function bar(percent) {
  const track = element("div", "bar");
  const fill = element("div", "bar-fill");
  fill.style.width = `${Math.min(percent, 100)}%`;
  fill.dataset.level = level(percent);
  track.append(fill);
  return track;
}

/**
 * One limit window: the percentage, a bar, the countdown, and where the
 * current rate is heading.
 */
function limitCard(label, window_, showForecast) {
  const card = element("section", "card");

  const head = element("div", "row");
  head.append(element("span", "label", label));
  const value = element("span", "value", `${Math.round(window_.percentUsed)}%`);
  value.dataset.level = level(window_.percentUsed);
  head.append(value);
  card.append(head, bar(window_.percentUsed));

  if (window_.resetsAt > 0) {
    // Marked so the one-second tick can rewrite it without a new snapshot.
    const countdown = element("div", "muted countdown");
    countdown.dataset.resetsAt = String(window_.resetsAt);
    card.append(countdown);
  }

  if (showForecast && window_.forecast) {
    card.append(forecastLine(window_.forecast));
  }
  return card;
}

/** The line that says where the current rate lands, not where it stands. */
function forecastLine(forecast) {
  if (forecast.exhaustedAt) {
    const line = element(
      "div",
      "forecast",
      t.runsOutAt(formatClock(forecast.exhaustedAt)),
    );
    line.dataset.level = "danger";
    return line;
  }
  return element(
    "div",
    "forecast",
    t.projected(Math.round(forecast.projectedPercent)),
  );
}

/**
 * Costs are what the traffic would bill at API rates, not what a subscription
 * is charged, so every place one is shown says so.
 */
function formatMoney(usd) {
  if (settings.currency.code === "VND") {
    const dong = usd * settings.currency.vndRate;
    return `${Math.round(dong).toLocaleString("vi-VN")}₫`;
  }
  return `$${usd.toFixed(2)}`;
}

/**
 * A tilde means at least one record used a model absent from the price table
 * and was billed at the Opus rate as an approximation.
 */
function maybeApproximate(text) {
  return snapshot.hasEstimatedPricing ? `~${text}` : text;
}

/** Today's spend, and whether that is unusual for this user. */
function costCard() {
  const card = element("section", "card");

  const head = element("div", "row");
  head.append(element("span", "label", t.today));
  head.append(
    element(
      "span",
      "value",
      maybeApproximate(formatMoney(snapshot.today.costUsd)),
    ),
  );
  card.append(head, element("div", "muted", t.atApiRates));

  const ratio = snapshot.todayVsAvg;
  if (ratio !== null && ratio !== undefined) {
    // A bare figure says nothing without the user's own baseline: their days
    // range from a few dollars to over a hundred.
    const wording =
      ratio >= 1
        ? t.timesAverage(ratio.toFixed(1))
        : t.percentOfAverage(Math.round(ratio * 100));
    const line = element("div", "muted", wording);
    if (ratio >= 2) line.dataset.level = "warn";
    card.append(line);
  }

  if (settings.cards.chart) card.append(chart());
  return card;
}

/**
 * Fourteen days as bars. Drawn with plain elements rather than a charting
 * library, which for fourteen values would cost more than it gives.
 */
function chart() {
  const peak = Math.max(...snapshot.days.map((day) => day.costUsd), 0);
  const wrap = element("div", "chart");

  for (const day of snapshot.days) {
    const column = element("div", "chart-col");
    const fill = element("div", "chart-bar");
    // A day with no usage keeps its column and shows an empty one, so the
    // fourteen days stay aligned to the same dates.
    fill.style.height = peak > 0 ? `${(day.costUsd / peak) * 100}%` : "0%";
    if (day.date === snapshot.today.date) fill.dataset.today = "true";
    column.title = `${day.date} · ${formatMoney(day.costUsd)}`;
    column.append(fill);
    wrap.append(column);
  }
  return wrap;
}

/** "2.5M", "148k", "920" — three significant figures is all that is read. */
function formatTokens(count) {
  if (count >= 1e6) return `${(count / 1e6).toFixed(1)}M`;
  if (count >= 1e3) return `${Math.round(count / 1e3)}k`;
  return String(count);
}

/** Short name for a model id, since the full one does not fit. */
function shortModel(id) {
  return id.replace(/^claude-/, "").replace(/\[.*\]$/, "");
}

/**
 * Which models today's cost went to. Worth showing because the tiers differ
 * by a factor of ten, so the mix explains an expensive day better than the
 * total does.
 */
function modelsCard() {
  const total = snapshot.byModel.reduce((sum, m) => sum + m.costUsd, 0);
  if (total <= 0) return null;

  const card = element("section", "card");
  card.append(element("div", "label", t.modelsToday));
  for (const model of snapshot.byModel.slice(0, 4)) {
    const row = element("div", "row");
    row.append(element("span", "muted", shortModel(model.model)));
    row.append(
      element("span", "muted", `${Math.round((model.costUsd / total) * 100)}%`),
    );
    card.append(row);
  }
  return card;
}

/**
 * Share of input volume served from cache. Normally around 99%; a fall means
 * something is invalidating the prompt prefix and quietly costing money.
 */
function cacheCard() {
  const card = element("section", "card");
  const row = element("div", "row");
  row.append(element("span", "label", t.cacheHitRate));
  const value = element(
    "span",
    "value",
    `${(snapshot.cacheHitRate * 100).toFixed(1)}%`,
  );
  if (snapshot.cacheHitRate < 0.9) value.dataset.level = "warn";
  row.append(value);
  card.append(row);
  return card;
}

function topProjectCard() {
  const top = snapshot.byProject[0];
  if (!top) return null;

  const card = element("section", "card");
  const row = element("div", "row");
  row.append(element("span", "label", top.project || t.unknownProject));
  row.append(element("span", "muted", formatMoney(top.costUsd)));
  card.append(row);
  return card;
}

function sessionCard() {
  const session = snapshot.currentSession;
  if (!session) return null;

  const card = element("section", "card");
  const row = element("div", "row");
  row.append(element("span", "label", t.currentSession));
  row.append(element("span", "muted", formatMoney(session.costUsd)));
  card.append(row);

  const tokens = session.tokens;
  card.append(
    element(
      "div",
      "muted",
      t.sessionTokens(
        formatTokens(tokens.output),
        formatTokens(tokens.cacheRead),
      ),
    ),
  );
  return card;
}

/**
 * A reading older than two poll intervals is late enough that something is
 * wrong, and the widget says how old it is rather than presenting it as now.
 */
function isStale() {
  if (!snapshot.limitsReadAt) return false;
  const age = nowSeconds() - snapshot.limitsReadAt;
  return age > settings.usageIntervalSeconds * 2;
}

/** Says the limits could not be read, and offers to try again now. */
function limitsErrorCard() {
  const card = element("section", "card");
  const row = element("div", "row");
  row.append(element("span", "label", t.limits));
  const state = element("span", "value small", t.limitsUnavailable);
  state.dataset.level = "warn";
  row.append(state);
  card.append(row);

  const { code, detail } = snapshot.limitsError;
  card.append(element("div", "muted", t.errors[code] ?? code));
  // Detail is the other program's own words, so it stays untranslated.
  if (detail) card.append(element("div", "muted detail", detail));

  const retry = element("button", "retry", t.retry);
  retry.addEventListener("click", () => {
    retry.disabled = true;
    retry.textContent = t.retrying;
    core.invoke("retry_usage").catch(() => {});
  });
  card.append(retry);
  return card;
}

/** Marks how old the shown limits are, so they are not read as current. */
function staleNote() {
  return element(
    "div",
    "muted note",
    t.readAt(formatClock(snapshot.limitsReadAt)),
  );
}

// -- rendering -------------------------------------------------------------

function render() {
  root.replaceChildren();

  if (!snapshot || !settings) {
    root.append(element("p", "empty", t.loading));
    return;
  }

  const cards = settings.cards;
  const limits = snapshot.limits;

  // Failing to read the limits is reported whether or not an older reading
  // survives, so the widget never shows a stale number as if it were current.
  if (settings.usageEnabled && snapshot.limitsError) {
    root.append(limitsErrorCard());
  }

  if (limits) {
    if (cards.session && limits.session) {
      root.append(limitCard(t.sessionWindow, limits.session, cards.forecast));
    }
    if (cards.weekAll && limits.weekAll) {
      root.append(limitCard(t.weekAll, limits.weekAll, cards.forecast));
    }
    if (cards.weekModel && limits.weekModel) {
      root.append(
        limitCard(
          t.weekModel(limits.weekModel.modelName),
          limits.weekModel,
          cards.forecast,
        ),
      );
    }
    if (isStale()) root.append(staleNote());
  }

  if (cards.costToday) root.append(costCard());

  // Each of these returns null when it has nothing to say, so a card that is
  // switched on but empty leaves no blank strip behind.
  const optional = [
    [cards.models, modelsCard],
    [cards.cacheHitRate, cacheCard],
    [cards.topProject, topProjectCard],
    [cards.currentSession, sessionCard],
  ];
  for (const [enabled, build] of optional) {
    if (!enabled) continue;
    const card = build();
    if (card) root.append(card);
  }

  if (!root.childElementCount) {
    root.append(element("p", "empty", t.nothingToShow));
  }

  applyOpacity();
  tick();
  fitWindow();
}

/**
 * Background opacity is a setting, so it is applied as a CSS variable rather
 * than baked into the stylesheet.
 */
function applyOpacity() {
  document.documentElement.style.setProperty(
    "--bg-alpha",
    String(settings.opacity),
  );
}

/**
 * Rewrites the countdowns. Runs every second so the time left stays honest
 * between polls, which are minutes apart.
 */
function tick() {
  if (!settings) return;
  const now = nowSeconds();
  for (const node of root.querySelectorAll(".countdown")) {
    const remaining = Number(node.dataset.resetsAt) - now;
    node.textContent =
      remaining > 0 ? t.remaining(formatDuration(remaining)) : t.resetting;
  }
}

/** Keeps the window the height of whatever cards are switched on. */
function fitWindow() {
  const height = Math.ceil(root.getBoundingClientRect().height);
  core.invoke("resize_widget", { width: 260, height }).catch(() => {});
}

/**
 * Makes the whole surface a drag handle, the way a desktop widget behaves.
 *
 * `data-tauri-drag-region` only covers the element carrying it, not its
 * children, and this panel is nothing but children — cards, labels, bars — so
 * the attribute left almost nowhere to grab. Anything a person can actually
 * click keeps its own behaviour.
 */
function enableDragging() {
  root.addEventListener("mousedown", async (event_) => {
    if (event_.button !== 0) return;
    if (event_.target.closest("button, a, input, select, textarea")) return;
    event_.preventDefault();
    try {
      await tauriWindow.getCurrentWindow().startDragging();
    } catch {
      // Dragging is a convenience; losing it must not break the widget.
    }
  });
}

function useSettings(next) {
  settings = next;
  t = stringsFor(settings.language);
}

async function main() {
  enableDragging();
  useSettings(await core.invoke("get_settings"));
  snapshot = await core.invoke("get_snapshot");
  render();

  await event.listen("snapshot", (e) => {
    snapshot = e.payload;
    render();
  });
  await event.listen("settings", (e) => {
    useSettings(e.payload);
    render();
  });
  setInterval(tick, 1000);
}

main();
