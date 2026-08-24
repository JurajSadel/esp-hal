const DEFAULT_ALLOWED = require("../chips.json").map((c) => c.soc);

function parsePackage(body) {
  const text = String(body || "").trim().toLowerCase();
  if (text.includes("hil-test-radio")) {
    return "hil-test-radio";
  }
  if (text.includes("hil-test")) {
    return "hil-test";
  }
  return "hil-test";
}

function parseTests(body) {
  const text = String(body || "").trim();
  const m = text.match(/--tests?\s+(.+)$/i);
  if (!m) return "";
  return m[1]
    .trim()
    .split(/[,\s]+/)
    .map((s) => s.trim())
    .filter(Boolean)
    .join(",");
}

function parseChips(body, allowed = DEFAULT_ALLOWED) {
  const body_trimmed = String(body || "").trim();

  // Remove the leading "/hil"
  const withoutCmd = body_trimmed.replace(/^\/hil\s+/i, "");

  // Split on commas and/or whitespace, normalize and dedupe
  const parts = withoutCmd
    .split(/[,\s]+/)
    .map((s) => s.toLowerCase().replace(/[,]+$/, ""))
    .filter(Boolean);

  const chips = Array.from(new Set(parts.filter((s) => allowed.includes(s))));

  if (!chips.length) {
    return {
      chips: "",
      chipsLabel: "",
      error:
        "No valid chips specified.\n\nAllowed chips are: " + allowed.join(", "),
    };
  }

  return {
    chips: chips.join(" "),
    chipsLabel: chips.join(", "),
    error: "",
  };
}

module.exports = { parseTests, parseChips, parsePackage };