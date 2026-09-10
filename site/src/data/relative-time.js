const RELATIVE_UNITS = [
  ["y", 31536000],
  ["mo", 2592000],
  ["d", 86400],
  ["h", 3600],
  ["m", 60],
];

// Compact form used by the activity feed - "4m ago", "2d ago".
export function formatRelativeTime(date) {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return "just now";

  for (const [suffix, unitSeconds] of RELATIVE_UNITS) {
    const value = Math.floor(seconds / unitSeconds);
    if (value >= 1) return `${value}${suffix} ago`;
  }
  return "just now";
}

const RELATIVE_UNITS_LONG = [
  ["year", 31536000],
  ["month", 2592000],
  ["day", 86400],
  ["hour", 3600],
  ["minute", 60],
];

// Spelled-out form ("4 minutes ago", "5 days ago") for places with room to show it in full.
export function formatRelativeTimeLong(date) {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return "just now";

  for (const [unit, unitSeconds] of RELATIVE_UNITS_LONG) {
    const value = Math.floor(seconds / unitSeconds);
    if (value >= 1) return `${value} ${unit}${value === 1 ? "" : "s"} ago`;
  }
  return "just now";
}
