// Reports OSRS Wiki bosses not yet represented in BOSS_COMBAT_LEVELS, for manual triage.
// Read-only: writes a report file but never touches boss-levels.js/notable_npcs.rs/etc.
import axios from 'axios';
import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SITE_ROOT = path.resolve(__dirname, '..', '..');
const BOSS_LEVELS_PATH = path.join(SITE_ROOT, 'src', 'data', 'boss-levels.js');
const REPORT_PATH = path.join(__dirname, 'new-bosses-report.json');

const USER_AGENT = 'GroupScape-DataPipeline/1.0 (contact: repo issue tracker)';
const WIKI_API = 'https://oldschool.runescape.wiki/api.php';
const WOM_ICON_BASE =
  'https://raw.githubusercontent.com/wise-old-man/wise-old-man/master/app/public/img/metrics';

function slugifyNpcName(name) {
  return name
    .replace(/'/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

async function getKnownSlugs() {
  const raw = await fs.readFile(BOSS_LEVELS_PATH, 'utf-8');
  const keys = [...raw.matchAll(/^\s*([a-z0-9_]+):\s*\d+,/gm)].map((m) => m[1]);
  return new Set(keys);
}

async function fetchCategoryMembers() {
  const members = [];
  let cmcontinue;
  do {
    const res = await axios.get(WIKI_API, {
      headers: { 'User-Agent': USER_AGENT },
      params: {
        action: 'query',
        list: 'categorymembers',
        cmtitle: 'Category:Bosses',
        cmlimit: 500,
        format: 'json',
        ...(cmcontinue ? { cmcontinue } : {}),
      },
    });
    const data = res.data;
    const batch = data?.query?.categorymembers;
    if (!Array.isArray(batch)) {
      throw new Error(`Unexpected categorymembers response shape: ${JSON.stringify(data).slice(0, 200)}`);
    }
    members.push(...batch);
    cmcontinue = data?.continue?.cmcontinue;
  } while (cmcontinue);
  return members;
}

async function fetchCombatLevel(title) {
  try {
    const res = await axios.get(WIKI_API, {
      headers: { 'User-Agent': USER_AGENT },
      params: { action: 'parse', page: title, prop: 'wikitext', section: 0, format: 'json' },
    });
    const wikitext = res.data?.parse?.wikitext?.['*'];
    if (!wikitext) return null;
    const match = wikitext.match(/\|\s*combat\s*=\s*([0-9]+)/);
    return match ? Number(match[1]) : null;
  } catch (err) {
    return null;
  }
}

async function hasWiseOldManIcon(slug) {
  try {
    const res = await axios.head(`${WOM_ICON_BASE}/${slug}.png`, { validateStatus: null });
    return res.status === 200;
  } catch (err) {
    return false;
  }
}

async function main() {
  const knownSlugs = await getKnownSlugs();
  const members = await fetchCategoryMembers();

  const candidates = [];
  for (const member of members) {
    const title = member.title;
    if (!title || member.ns !== 0) continue;
    const slug = slugifyNpcName(title);
    if (knownSlugs.has(slug)) continue;

    const combatLevel = await fetchCombatLevel(title);
    const iconExists = await hasWiseOldManIcon(slug);
    candidates.push({
      title,
      slug,
      combatLevel,
      wikiUrl: `https://oldschool.runescape.wiki/w/${encodeURIComponent(title.replace(/ /g, '_'))}`,
      hasWiseOldManIcon: iconExists,
    });
  }

  await fs.writeFile(REPORT_PATH, JSON.stringify(candidates, null, 2));
  console.log(JSON.stringify(candidates, null, 2));
  console.log(`\n${candidates.length} candidate bosses not in BOSS_COMBAT_LEVELS.`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
