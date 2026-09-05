// Reports OSRS Wiki combat achievement task pages not yet represented in
// combat_achievement_tasks.json, for manual triage. Read-only: writes a report file but never
// touches combat_achievement_tasks.json itself.
//
// Source: every wiki page transcluding Template:Infobox Combat Achievement (list=embeddedin) -
// each task has its own page under that infobox, same pattern as the sibling
// detect-new-quests.mjs/detect-new-bosses.mjs use for their own infobox templates.
import axios from "axios";
import fs from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");
const CA_TASKS_PATH = path.join(REPO_ROOT, "server", "src", "content", "combat_achievement_tasks.json");
const REPORT_PATH = path.join(__dirname, "new-ca-tasks-report.json");

const USER_AGENT = "GroupScape-DataPipeline/1.0 (contact: repo issue tracker)";
const WIKI_API = "https://oldschool.runescape.wiki/api.php";
const INFOBOX_TEMPLATE = "Template:Infobox Combat Achievement";

async function getKnownTaskNames() {
  const raw = await fs.readFile(CA_TASKS_PATH, "utf-8");
  const data = JSON.parse(raw);
  const names = new Set();
  for (const tasks of Object.values(data)) {
    for (const task of tasks) names.add(task.name);
  }
  return names;
}

async function fetchTaskPages() {
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
  const knownNames = await getKnownTaskNames();
  const pages = await fetchTaskPages();

  const candidates = pages
    .map((page) => page.title)
    .filter((title) => !knownNames.has(title))
    .map((title) => ({
      title,
      wikiUrl: `https://oldschool.runescape.wiki/w/${encodeURIComponent(title.replace(/ /g, "_"))}`,
    }));

  await fs.writeFile(REPORT_PATH, JSON.stringify(candidates, null, 2));
  console.log(JSON.stringify(candidates, null, 2));
  console.log(
    `\n${candidates.length} candidate combat achievement tasks not in combat_achievement_tasks.json (out of ${pages.length} on the wiki).`
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
