// Reports OSRS Wiki quest pages not yet represented in quest_data.json, for manual triage.
// Read-only: writes a report file but never touches quest_data.json/quest_ids.rs.
//
// Source: every wiki page transcluding Template:Infobox Quest (list=embeddedin) - a quest's own
// infobox is a more reliable signal than Category:Quests, which mostly holds meta/list pages
// rather than the quests themselves (see the sibling detect-new-bosses.mjs for the same pattern
// against Category:Bosses' infobox instead).
import axios from "axios";
import fs from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SITE_ROOT = path.resolve(__dirname, "..", "..");
const QUEST_DATA_PATH = path.join(SITE_ROOT, "public", "data", "quest_data.json");
const REPORT_PATH = path.join(__dirname, "new-quests-report.json");

const USER_AGENT = "GroupScape-DataPipeline/1.0 (contact: repo issue tracker)";
const WIKI_API = "https://oldschool.runescape.wiki/api.php";
const INFOBOX_TEMPLATE = "Template:Infobox Quest";

async function getKnownQuestNames() {
  const raw = await fs.readFile(QUEST_DATA_PATH, "utf-8");
  const data = JSON.parse(raw);
  return new Set(Object.values(data).map((entry) => entry.name));
}

async function fetchQuestPages() {
  const pages = [];
  let eicontinue;
  do {
    const res = await axios.get(WIKI_API, {
      headers: { "User-Agent": USER_AGENT },
      params: {
        action: "query",
        list: "embeddedin",
        eititle: INFOBOX_TEMPLATE,
        einamespace: 0,
        eilimit: 500,
        format: "json",
        ...(eicontinue ? { eicontinue } : {}),
      },
    });
    const data = res.data;
    const batch = data?.query?.embeddedin;
    if (!Array.isArray(batch)) {
      throw new Error(`Unexpected embeddedin response shape: ${JSON.stringify(data).slice(0, 200)}`);
    }
    pages.push(...batch);
    eicontinue = data?.continue?.eicontinue;
  } while (eicontinue);
  return pages;
}

async function main() {
  const knownNames = await getKnownQuestNames();
  const pages = await fetchQuestPages();

  const candidates = pages
    .map((page) => page.title)
    // Subpages (e.g. "Recipe for Disaster/Freeing Pirate Pete") are per-subquest guide pages that
    // share their parent quest's infobox - RuneLite (and this app) tracks the parent as one quest,
    // so a subpage is never itself a missing quest.
    .filter((title) => !title.includes("/"))
    .filter((title) => !knownNames.has(title))
    .map((title) => ({
      title,
      wikiUrl: `https://oldschool.runescape.wiki/w/${encodeURIComponent(title.replace(/ /g, "_"))}`,
    }));

  await fs.writeFile(REPORT_PATH, JSON.stringify(candidates, null, 2));
  console.log(JSON.stringify(candidates, null, 2));
  console.log(`\n${candidates.length} candidate quests not in quest_data.json (out of ${pages.length} on the wiki).`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
