// Builds server/src/content/item_wiki_icons.json - a verified item id -> wiki icon filename map,
// used by the server's Discord notifications (`discord::item_icon_url`) to link straight to the
// wiki's own image instead of guessing a filename from the item's display name at request time.
//
// The guess ("<Name>.png") is right most of the time, but the wiki's own File: namespace is the
// source of truth: some items' real file is a differently-named redirect (e.g. "Coins.png"
// redirects to "Coins 100.png"; "Zulrah's scales.png" to "Zulrah's scales 5.png"), and a handful
// have no icon file at all. This script resolves every item name once via the MediaWiki API
// (`imageinfo` on the `File:` page, following redirects) and caches the *verified* filename, so
// the server never has to guess. An item missing from the output map just falls back to the
// existing name-guess at request time (see `discord::item_icon_url`) - not a regression, since
// that's all it does today.
import axios from "axios";
import fs from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SITE_ROOT = path.resolve(__dirname, "..", "..");
const REPO_ROOT = path.resolve(SITE_ROOT, "..");
const ITEM_DATA_PATH = path.join(SITE_ROOT, "public", "data", "item_data.json");
const OUTPUT_PATH = path.join(REPO_ROOT, "server", "src", "content", "item_wiki_icons.json");

const USER_AGENT = "GroupScape-DataPipeline/1.0 (contact: repo issue tracker)";
const API_URL = "https://oldschool.runescape.wiki/api.php";
const BATCH_SIZE = 50;
const BATCH_DELAY_MS = 500;
const MAX_RETRIES = 3;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function fileTitle(name) {
  return `File:${name}.png`;
}

// Resolves each requested `File:<name>.png` title through the API's normalization + redirect
// chains to the final title actually returned in `pages` - both lists are flat single-hop lists
// (from -> to), so this just keeps re-applying matches until nothing more applies. In practice
// that's at most one of each, but nothing here assumes so.
function resolveFinalTitles(originalTitles, normalized, redirects) {
  const hops = [...(normalized || []), ...(redirects || [])];
  const finalTitleOf = new Map();
  for (const title of originalTitles) {
    let current = title;
    let steps = 0;
    while (steps++ < hops.length + 1) {
      const hop = hops.find((h) => h.from === current);
      if (!hop) break;
      current = hop.to;
    }
    finalTitleOf.set(title, current);
  }
  return finalTitleOf;
}

async function queryBatch(names, failures) {
  const titles = names.map(fileTitle);
  for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
    try {
      const res = await axios.get(API_URL, {
        params: {
          action: "query",
          format: "json",
          formatversion: 2,
          redirects: 1,
          prop: "imageinfo",
          iiprop: "timestamp",
          titles: titles.join("|"),
        },
        headers: { "User-Agent": USER_AGENT },
      });

      const { pages = [], normalized, redirects } = res.data.query || {};
      const finalTitleOf = resolveFinalTitles(titles, normalized, redirects);
      const filenameByFinalTitle = new Map();
      for (const page of pages) {
        if (page.missing) continue;
        filenameByFinalTitle.set(page.title, page.title.replace(/^File:/, ""));
      }

      const results = new Map();
      for (const name of names) {
        const finalTitle = finalTitleOf.get(fileTitle(name));
        const filename = filenameByFinalTitle.get(finalTitle);
        if (filename) results.set(name, filename);
      }
      return results;
    } catch (err) {
      const status = err.response?.status;
      if (status === 429 && attempt < MAX_RETRIES) {
        await sleep(1000 * 2 ** attempt);
        continue;
      }
      failures.push({ names, error: err.message });
      return new Map();
    }
  }
  return new Map();
}

async function main() {
  const itemData = JSON.parse(await fs.readFile(ITEM_DATA_PATH, "utf-8"));
  const uniqueNames = [...new Set(Object.values(itemData).map((entry) => entry.name))];

  const nameToFilename = new Map();
  const failures = [];
  for (let i = 0; i < uniqueNames.length; i += BATCH_SIZE) {
    const batch = uniqueNames.slice(i, i + BATCH_SIZE);
    const results = await queryBatch(batch, failures);
    for (const [name, filename] of results) nameToFilename.set(name, filename);
    if ((i / BATCH_SIZE) % 10 === 0) {
      console.log(`  ${Math.min(i + BATCH_SIZE, uniqueNames.length)}/${uniqueNames.length} names resolved`);
    }
    await sleep(BATCH_DELAY_MS);
  }

  const idToFilename = {};
  for (const [id, entry] of Object.entries(itemData)) {
    const filename = nameToFilename.get(entry.name);
    if (filename) idToFilename[id] = filename;
  }

  await fs.writeFile(OUTPUT_PATH, JSON.stringify(idToFilename));

  console.log(`Unique names queried: ${uniqueNames.length}`);
  console.log(`Names resolved to a verified icon: ${nameToFilename.size}`);
  console.log(`Item ids mapped: ${Object.keys(idToFilename).length}/${Object.keys(itemData).length}`);
  if (failures.length) {
    console.log(`Batch failures: ${failures.length}`);
    for (const f of failures) {
      console.log(`  [${f.names.join(", ")}]: ${f.error}`);
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
