/**
 * Wording for both windows, in both languages.
 *
 * Vietnamese keeps the English keyword where translating it would make it
 * harder to recognise — cache, model, reset, session id — and describes the
 * behaviour plainly everywhere else. No word here should need a second read.
 *
 * Wrapped in a function so only CUW_STRINGS reaches the page. These are plain
 * scripts, not modules, so anything declared at the top level here would
 * collide with the same name in the script that consumes it.
 */
(() => {
const STRINGS = {
  vi: {
    loading: "Đang đọc dữ liệu…",
    nothingToShow: "Chưa có gì để hiện",

    // Cửa sổ hạn mức. Kèm độ dài cửa sổ vì đó là thứ quyết định con số.
    sessionWindow: "Phiên 5 giờ",
    weekAll: "Tuần · mọi model",
    weekModel: (model) => `Tuần · ${model}`,
    remaining: (duration) => `còn ${duration}`,
    resetting: "đang reset",
    projected: (percent) => `theo nhịp hiện tại: ${percent}% khi reset`,
    runsOutAt: (clock) => `hết hạn mức lúc ${clock}`,

    today: "Hôm nay",
    atApiRates: "quy đổi theo giá API",
    timesAverage: (ratio) => `gấp ${ratio} lần mức trung bình 7 ngày`,
    percentOfAverage: (percent) => `bằng ${percent}% mức trung bình 7 ngày`,

    modelsToday: "Model hôm nay",
    cacheHitRate: "Tỉ lệ đọc từ cache",
    unknownProject: "(không rõ project)",
    currentSession: "Phiên đang mở",
    sessionTokens: (output, cacheRead) =>
      `${output} output · ${cacheRead} đọc cache`,

    limits: "Hạn mức",
    limitsUnavailable: "không đọc được",
    retry: "Đọc lại",
    retrying: "Đang đọc…",
    readAt: (clock) => `đọc lúc ${clock}`,

    errors: {
      "not-logged-in":
        "Chưa đăng nhập Claude Code, hoặc tài khoản không dùng gói subscription. Chạy `claude` trong terminal rồi đăng nhập.",
      "report-changed":
        "Báo cáo /usage không còn dòng hạn mức nào. Có thể Anthropic đã đổi câu chữ.",
      "command-not-found":
        "Không tìm thấy lệnh `claude`. Chạy `which claude` trong terminal rồi dán đường dẫn vào ô đường dẫn ở cửa sổ cấu hình.",
      "timed-out": "Lệnh `claude` không trả lời kịp.",
      "command-failed": "Lệnh `claude` chạy lỗi.",
      "unreadable-output": "Không đọc được kết quả lệnh `claude` trả về.",
    },

    settings: {
      title: "Cấu hình",
      groupDisplay: "Hiển thị",
      groupSources: "Nguồn dữ liệu",
      groupAlerts: "Cảnh báo",
      groupWindow: "Cửa sổ",

      language: "Ngôn ngữ",
      trayMetric: "Số hiện ở khay",
      trayMetricSession: "Phần trăm phiên",
      trayMetricWeek: "Phần trăm tuần",
      trayMetricBoth: "Cả hai, xếp chồng",
      trayMetricCost: "Tiền hôm nay",
      trayMetricTokens: "Token hôm nay",
      currency: "Đơn vị tiền",
      exchangeRate: "Tỉ giá 1 USD",

      usageEnabled: "Đọc hạn mức bằng lệnh `claude -p /usage`",
      usageEnabledHint:
        "Tắt thì chỉ còn phần tiền quy đổi từ transcript. Lệnh này không tốn token.",
      transcriptInterval: "Nhịp đọc transcript (giây)",
      usageInterval: "Nhịp chạy /usage (giây)",
      usageIntervalHint: "Mỗi lần chạy mất khoảng 3 giây khởi động CLI.",
      claudePath: "Đường dẫn tới lệnh `claude`",
      claudePathHint:
        "Để trống thì app tự tìm ở những chỗ cài thông dụng. Chỉ điền khi app báo không tìm thấy: chạy `which claude` trong terminal rồi dán kết quả vào đây.",

      notificationsEnabled: "Báo khi vượt mức",
      sessionAlert: "Mức cảnh báo phiên (%)",
      weekAlert: "Mức cảnh báo tuần (%)",

      alwaysOnTop: "Luôn nằm trên cửa sổ khác",
      skipTaskbar: "Không hiện trên taskbar",
      autostart: "Mở cùng hệ điều hành",
      opacity: "Độ đậm nền",
      saved: "Đã lưu",

      cards: {
        session: "Phần trăm phiên",
        forecast: "Dự đoán hết hạn mức",
        weekAll: "Phần trăm tuần chung",
        weekModel: "Phần trăm tuần theo model",
        costToday: "Tiền hôm nay",
        chart: "Biểu đồ 14 ngày",
        models: "Tỉ lệ model",
        cacheHitRate: "Tỉ lệ đọc từ cache",
        topProject: "Project tốn nhất",
        currentSession: "Phiên đang mở",
      },
    },
  },

  en: {
    loading: "Reading…",
    nothingToShow: "Nothing to show",

    sessionWindow: "5-hour session",
    weekAll: "Week · all models",
    weekModel: (model) => `Week · ${model}`,
    remaining: (duration) => `${duration} left`,
    resetting: "resetting",
    projected: (percent) => `at this rate: ${percent}% by reset`,
    runsOutAt: (clock) => `runs out at ${clock}`,

    today: "Today",
    atApiRates: "at API rates",
    timesAverage: (ratio) => `${ratio}× the 7-day average`,
    percentOfAverage: (percent) => `${percent}% of the 7-day average`,

    modelsToday: "Models today",
    cacheHitRate: "Cache hit rate",
    unknownProject: "(unknown project)",
    currentSession: "Current session",
    sessionTokens: (output, cacheRead) =>
      `${output} output · ${cacheRead} cache read`,

    limits: "Limits",
    limitsUnavailable: "unavailable",
    retry: "Retry",
    retrying: "Reading…",
    readAt: (clock) => `read at ${clock}`,

    errors: {
      "not-logged-in":
        "Not signed in to Claude Code, or this account has no subscription. Run `claude` in a terminal and sign in.",
      "report-changed":
        "The /usage report no longer contains a limit line. Anthropic may have reworded it.",
      "command-not-found":
        "The `claude` command could not be found. Run `which claude` in a terminal and paste the path into the settings window.",
      "timed-out": "The `claude` command did not answer in time.",
      "command-failed": "The `claude` command failed.",
      "unreadable-output": "Could not read what the `claude` command returned.",
    },

    settings: {
      title: "Settings",
      groupDisplay: "Display",
      groupSources: "Data sources",
      groupAlerts: "Alerts",
      groupWindow: "Window",

      language: "Language",
      trayMetric: "Number in the tray",
      trayMetricSession: "Session percentage",
      trayMetricWeek: "Weekly percentage",
      trayMetricBoth: "Both, stacked",
      trayMetricCost: "Today's cost",
      trayMetricTokens: "Today's tokens",
      currency: "Currency",
      exchangeRate: "Rate per USD",

      usageEnabled: "Read limits with `claude -p /usage`",
      usageEnabledHint:
        "Turn off to keep only the cost converted from transcripts. The command spends no tokens.",
      transcriptInterval: "Transcript interval (seconds)",
      usageInterval: "/usage interval (seconds)",
      usageIntervalHint: "Each run costs about 3 seconds of CLI startup.",
      claudePath: "Path to the `claude` command",
      claudePathHint:
        "Left empty, the app searches the usual install locations. Fill it in only when the app reports it cannot find the command: run `which claude` in a terminal and paste the result here.",

      notificationsEnabled: "Notify when a limit is passed",
      sessionAlert: "Session threshold (%)",
      weekAlert: "Weekly threshold (%)",

      alwaysOnTop: "Keep above other windows",
      skipTaskbar: "Hide from the taskbar",
      autostart: "Start with the system",
      opacity: "Background opacity",
      saved: "Saved",

      cards: {
        session: "Session percentage",
        forecast: "Forecast of running out",
        weekAll: "Weekly percentage, all models",
        weekModel: "Weekly percentage per model",
        costToday: "Today's cost",
        chart: "14-day chart",
        models: "Model mix",
        cacheHitRate: "Cache hit rate",
        topProject: "Most expensive project",
        currentSession: "Current session",
      },
    },
  },
};

/** Falls back to Vietnamese for an unknown code rather than rendering blanks. */
function stringsFor(language) {
  return STRINGS[language] ?? STRINGS.vi;
}

/**
 * Clock format follows the language: Vietnamese reads a 24-hour clock, English
 * a 12-hour one. Left explicit rather than taken from the OS locale, which is
 * often English on a machine whose owner wants Vietnamese.
 */
function formatClockFor(language, unixSeconds) {
  const vietnamese = language === "vi";
  return new Date(unixSeconds * 1000).toLocaleTimeString(
    vietnamese ? "vi-VN" : "en-US",
    {
      // A leading zero belongs on a 24-hour clock and looks wrong on a
      // 12-hour one.
      hour: vietnamese ? "2-digit" : "numeric",
      minute: "2-digit",
      hour12: !vietnamese,
    },
  );
}

  window.CUW_STRINGS = { stringsFor, formatClockFor };
})();
