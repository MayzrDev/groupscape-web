// Refreshes site/public/data/item_data.json (and the server's copy) from the OSRS Wiki's
// item mapping endpoint, plus downloads webp icons for anything new/changed.
//
// The mapping endpoint only covers GE-tradeable items (~4000 of ~19500 entries) - non-tradeable
// items already in item_data.json are left untouched since the wiki mapping has no data for them.
import axios from 'axios';
import fs from 'fs/promises';
import path from 'path';
import sharp from 'sharp';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SITE_ROOT = path.resolve(__dirname, '..', '..');
const REPO_ROOT = path.resolve(SITE_ROOT, '..');
const ITEM_DATA_PATH = path.join(SITE_ROOT, 'public', 'data', 'item_data.json');
const SERVER_ITEM_NAMES_PATH = path.join(REPO_ROOT, 'server', 'src', 'content', 'item_names.json');
const ICONS_DIR = path.join(SITE_ROOT, 'public', 'icons', 'items');

const USER_AGENT = 'GroupScape-DataPipeline/1.0 (contact: repo issue tracker)';
const MAPPING_URL = 'https://prices.runescape.wiki/api/v1/osrs/mapping';
const ICON_CONCURRENCY = 4;
const ICON_DELAY_MS = 500;
const ICON_MAX_RETRIES = 3;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function fetchMapping() {
  const res = await axios.get(MAPPING_URL, { headers: { 'User-Agent': USER_AGENT } });
  if (!Array.isArray(res.data)) {
    throw new Error(`Unexpected mapping response shape: ${typeof res.data}`);
  }
  return res.data;
}

async function downloadIcon(itemId, iconFilename, failures) {
  const dest = path.join(ICONS_DIR, `${itemId}.webp`);
  const url = `https://oldschool.runescape.wiki/w/Special:FilePath/${encodeURIComponent(iconFilename)}`;
  for (let attempt = 0; attempt <= ICON_MAX_RETRIES; attempt++) {
    try {
      const res = await axios.get(url, {
        headers: { 'User-Agent': USER_AGENT },
        responseType: 'arraybuffer',
      });
      const webp = await sharp(res.data).webp().toBuffer();
      await fs.writeFile(dest, webp);
      return true;
    } catch (err) {
      const status = err.response?.status;
      if (status === 429 && attempt < ICON_MAX_RETRIES) {
        await sleep(1000 * 2 ** attempt);
        continue;
      }
      failures.push({ itemId, iconFilename, error: err.message });
      return false;
    }
  }
  return false;
}

async function downloadIconsInBatches(entries, failures) {
  let downloaded = 0;
  for (let i = 0; i < entries.length; i += ICON_CONCURRENCY) {
    const batch = entries.slice(i, i + ICON_CONCURRENCY);
    const results = await Promise.all(
      batch.map(({ itemId, iconFilename }) => downloadIcon(itemId, iconFilename, failures))
    );
    downloaded += results.filter(Boolean).length;
    await sleep(ICON_DELAY_MS);
  }
  return downloaded;
}

async function main() {
  const currentRaw = await fs.readFile(ITEM_DATA_PATH, 'utf-8');
  const current = JSON.parse(currentRaw);

  const mapping = await fetchMapping();

  const added = [];
  const updated = [];
  const iconTargets = [];

  for (const entry of mapping) {
    const id = String(entry.id);
    const existing = current[id];
    const mappedHighalch = entry.highalch ?? 0;
    const nameChanged = !existing || existing.name !== entry.name;
    const alchChanged = !existing || existing.highalch !== mappedHighalch;

    if (!existing) {
      added.push(id);
    } else if (nameChanged || alchChanged) {
      updated.push(id);
    } else {
      continue;
    }

    const newEntry = { name: entry.name, highalch: entry.highalch ?? 0 };
    if (existing && existing.stacks) {
      newEntry.stacks = existing.stacks;
    }
    current[id] = newEntry;

    if (entry.icon) {
      iconTargets.push({ itemId: id, iconFilename: entry.icon });
      if (newEntry.stacks) {
        for (const [, stackId] of newEntry.stacks) {
          iconTargets.push({ itemId: stackId, iconFilename: entry.icon });
        }
      }
    }
  }

  const failures = [];
  const iconsDownloaded = await downloadIconsInBatches(iconTargets, failures);

  const serialized = JSON.stringify(current);
  await fs.writeFile(ITEM_DATA_PATH, serialized);
  await fs.writeFile(SERVER_ITEM_NAMES_PATH, serialized);

  console.log(`New items: ${added.length}`);
  if (added.length) console.log(`  ${added.join(', ')}`);
  console.log(`Updated items: ${updated.length}`);
  if (updated.length) console.log(`  ${updated.join(', ')}`);
  console.log(`Icons downloaded: ${iconsDownloaded}/${iconTargets.length}`);
  if (failures.length) {
    console.log(`Icon download failures: ${failures.length}`);
    for (const f of failures) {
      console.log(`  item ${f.itemId} (${f.iconFilename}): ${f.error}`);
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
