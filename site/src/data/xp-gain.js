// Pure XP-gain-over-a-period math, shared by the Graphs tab's chart (skill-graph.js) and its
// leaderboard sidepanel (skills-graphs.js) so both read the exact same numbers for the exact
// same selected period, instead of the sidepanel querying a coarser server-side window.

export const hourlyPeriods = new Set(["Hour1", "Hour6", "Hour12", "Day"]);

export function normalizedPeriod(period) {
  const periods = new Set(["Hour1", "Hour6", "Hour12", "Day", "Week", "Month", "Year"]);
  return periods.has(period) ? period : "Day";
}

export function truncatedDateForPeriod(date, period) {
  const normalized = normalizedPeriod(period);
  const t = new Date(date);
  t.setMinutes(0, 0, 0);

  if (!hourlyPeriods.has(normalized)) {
    t.setHours(0);
  }

  if (normalized === "Year") {
    t.setMonth(t.getMonth(), 1);
  }

  return t;
}

export function datesForPeriod(period) {
  const normalized = normalizedPeriod(period);
  const stepCountsForPeriods = {
    Hour1: 1,
    Hour6: 6,
    Hour12: 12,
    Day: 24,
    Week: 7,
    Month: 30,
    Year: 12,
  };
  const count = stepCountsForPeriods[normalized];
  const now = truncatedDateForPeriod(new Date(), normalized);
  const result = [];

  for (let i = count - 1; i >= 0; --i) {
    const t = new Date(now);

    if (hourlyPeriods.has(normalized)) {
      t.setTime(now.getTime() - i * 3600000);
      result.push(t);
      continue;
    }

    if (normalized === "Week" || normalized === "Month") {
      t.setDate(now.getDate() - i);
    } else if (normalized === "Year") {
      t.setMonth(now.getMonth() - i, 1);
    }

    result.push(truncatedDateForPeriod(t, normalized));
  }

  return result;
}

// Bucketing/forward-fill for a series of {time, data} snapshots. `valueFn` extracts the
// skill's xp from a raw snapshot; `currentValue` overrides the final bucket so the series
// always ends on the live current XP rather than a stale snapshot.
export function generateCompleteTimeSeries(series, currentValue, valueFn, dates, period) {
  const bucketedData = new Map();
  const earliestDateInPeriod = truncatedDateForPeriod(dates[0], period);
  const datesOutsideOfPeriod = [];
  for (const item of series) {
    const date = truncatedDateForPeriod(item.time, period);
    if (!bucketedData.has(date.getTime())) {
      bucketedData.set(date.getTime(), valueFn(item));
    }

    if (date < earliestDateInPeriod) {
      datesOutsideOfPeriod.push(item);
    }
  }

  let lastData = datesOutsideOfPeriod.length ? valueFn(datesOutsideOfPeriod[0]) : undefined;
  const result = [];

  for (const date of dates) {
    const time = date.getTime();
    if (bucketedData.has(time)) {
      const data = bucketedData.get(time);
      result.push(data);
      lastData = data;
    } else {
      result.push(lastData);
    }
  }

  result[result.length - 1] = currentValue;
  return result;
}

export function diffSeries(completeTimeSeries) {
  const changeData = [0];
  const cumulativeChangeData = [0];

  let s = 0;
  for (let i = 1; i < completeTimeSeries.length; ++i) {
    const previous = completeTimeSeries[i - 1];
    const current = completeTimeSeries[i];
    if (previous === undefined || current === undefined) {
      changeData.push(0);
      cumulativeChangeData.push(s);
    } else {
      changeData.push(current - previous);
      s += current - previous;
      cumulativeChangeData.push(s);
    }
  }

  return [completeTimeSeries, changeData, cumulativeChangeData];
}

// XP gained by each member over `period` for `skillName`, computed from the same
// skillDataForGroup/groupData the chart already fetched - no separate leaderboard API call,
// so the sidepanel can't drift from what the chart displays.
export function computeXpGains(skillDataForGroup, groupData, skillName, period) {
  const dates = datesForPeriod(period);
  const results = [];

  for (const playerSkillData of skillDataForGroup || []) {
    const member = groupData.members.get(playerSkillData.name);
    if (!member || !member.skills || !member.skills[skillName]) continue;

    const currentValue = member.skills[skillName].xp;
    const completeTimeSeries = generateCompleteTimeSeries(
      playerSkillData.skill_data,
      currentValue,
      (skillData) => skillData.data[skillName],
      dates,
      period
    );
    const [, , cumulativeChangeData] = diffSeries(completeTimeSeries);

    let xpGain = cumulativeChangeData[cumulativeChangeData.length - 1];
    if (Number.isNaN(xpGain)) xpGain = 0;

    results.push({ member_name: playerSkillData.name, value: xpGain });
  }

  return results;
}

// Standard competition ranking (1, 1, 3 - not 1, 1, 2), ties broken alphabetically by member
// name. Mirrors the server's `rank_entries` (leaderboard.rs) so ties read the same way.
export function rankEntries(entries) {
  const sorted = [...entries].sort((a, b) => b.value - a.value || a.member_name.localeCompare(b.member_name));

  const ranked = [];
  let rank = 0;
  let lastValue;
  sorted.forEach((entry, index) => {
    if (lastValue !== entry.value) {
      rank = index + 1;
      lastValue = entry.value;
    }
    ranked.push({ ...entry, rank });
  });

  return ranked;
}
