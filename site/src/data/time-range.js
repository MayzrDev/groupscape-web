// Shared first-event -> last-event span formatting, used by both the loot-log-page summary
// (across the whole scoped window) and loot-log-group headers (per source, so a boss killed
// across multiple days still shows a meaningful range instead of just a kill count).

export function timeBounds(rows) {
  let first = Infinity;
  let last = -Infinity;
  for (const row of rows) {
    const time = new Date(row.occurred_at).getTime();
    if (time < first) first = time;
    if (time > last) last = time;
  }
  return { first: new Date(first), last: new Date(last) };
}

export function formatDuration(first, last) {
  const minutes = Math.max(0, Math.round((last - first) / 60000));
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  if (hours < 24) return `${hours}h ${remainingMinutes}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

export function formatTimeRange(first, last) {
  const timeFormat = { hour: "numeric", minute: "2-digit" };
  const dateTimeFormat = { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" };
  if (first.getTime() === last.getTime()) {
    return first.toLocaleString([], dateTimeFormat);
  }
  const sameDay = first.toDateString() === last.toDateString();
  if (sameDay) {
    return `${first.toLocaleTimeString([], timeFormat)} &ndash; ${last.toLocaleTimeString([], timeFormat)}`;
  }
  return `${first.toLocaleString([], dateTimeFormat)} &ndash; ${last.toLocaleString([], dateTimeFormat)}`;
}
