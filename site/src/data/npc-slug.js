// Mirrors the server's slugify_npc_name (server/src/drop_rates.rs), so anything slugified
// client-side (boss filter options, hiscore icon lookups) lines up with the same slug the
// backend uses for filtering/matching.
export function slugifyNpcName(name) {
  return name
    .replace(/'/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}
