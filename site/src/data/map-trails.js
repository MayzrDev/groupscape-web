// Per-viewer movement history for group members shown on the map. Client-only: never
// sent to or read from the server. Persisted to localStorage so a reload doesn't lose
// the last few minutes of trail.

const STORAGE_KEY = "map-trails";
const SETTINGS_KEY = "map-trail-settings";

const DEFAULT_MAP_TRAIL_SETTINGS = {
  maxPoints: 20,
  maxAgeMs: 10 * 60 * 1000,
  jumpThresholdTiles: 8,
};

class MapTrails {
  constructor() {
    this.settings = this.loadSettings();
    this.trails = this.loadTrails();
  }

  loadSettings() {
    try {
      const stored = JSON.parse(localStorage.getItem(SETTINGS_KEY));
      return { ...DEFAULT_MAP_TRAIL_SETTINGS, ...stored };
    } catch {
      return { ...DEFAULT_MAP_TRAIL_SETTINGS };
    }
  }

  setSettings(partial) {
    this.settings = { ...this.settings, ...partial };
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(this.settings));
    } catch {}
  }

  loadTrails() {
    const trails = new Map();
    const now = Date.now();
    try {
      const stored = JSON.parse(localStorage.getItem(STORAGE_KEY));
      for (const [name, points] of Object.entries(stored || {})) {
        const pruned = this.prune(points, now);
        if (pruned.length > 0) trails.set(name, pruned);
      }
    } catch {}
    return trails;
  }

  persist() {
    const obj = {};
    for (const [name, points] of this.trails) obj[name] = points;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(obj));
    } catch {}
  }

  prune(points, now) {
    const { maxPoints, maxAgeMs } = this.settings;
    let pruned = (points || []).filter((p) => now - p.t <= maxAgeMs);
    if (pruned.length > maxPoints) pruned = pruned.slice(pruned.length - maxPoints);
    return pruned;
  }

  record(name, coordinates) {
    const now = Date.now();
    const points = this.trails.get(name) || [];
    const last = points[points.length - 1];
    if (last && last.x === coordinates.x && last.y === coordinates.y && last.plane === coordinates.plane) {
      return;
    }
    points.push({ x: coordinates.x, y: coordinates.y, plane: coordinates.plane, t: now });
    this.trails.set(name, this.prune(points, now));
    this.persist();
  }

  get(name) {
    const points = this.trails.get(name);
    if (!points || points.length === 0) return [];
    const pruned = this.prune(points, Date.now());
    if (pruned.length !== points.length) {
      this.trails.set(name, pruned);
      this.persist();
    }
    return pruned;
  }

  // Consecutive-point segments for a member's trail, oldest first. `isJump` marks a
  // discontinuity (teleport or floor change) that wasn't walked; `crossPlane` marks a
  // segment whose endpoints sit on different planes, which the map can only render as a
  // stub on whichever endpoint's plane is currently displayed.
  segmentsFor(name) {
    const points = this.get(name);
    const { jumpThresholdTiles } = this.settings;
    const segments = [];
    for (let i = 1; i < points.length; i++) {
      const a = points[i - 1];
      const b = points[i];
      const crossPlane = a.plane !== b.plane;
      const dist = Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
      segments.push({ a, b, isJump: crossPlane || dist > jumpThresholdTiles, crossPlane });
    }
    return segments;
  }
}

export const mapTrails = new MapTrails();

export { DEFAULT_MAP_TRAIL_SETTINGS };
