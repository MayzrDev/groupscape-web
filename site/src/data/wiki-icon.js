// Resolves a wiki page's thumbnail image via MediaWiki's pageimages API, rather than guessing a
// "<Name> chathead.png" file title - most bosses don't have a chathead file (no in-game dialogue),
// but every page has a pageimage. `origin=*` opts into the wiki's CORS allowlist for anonymous
// cross-origin reads (no auth, no write access) - see https://www.mediawiki.org/wiki/API:Cross-site_requests.
const WIKI_API = "https://oldschool.runescape.wiki/api.php";

// Keyed by page title; caches the in-flight/resolved promise so concurrent lookups for the same
// boss (e.g. several loot-log-group tiles mounting at once) share one request.
const iconUrlCache = new Map();

export function wikiPageIconUrl(pageTitle) {
  if (iconUrlCache.has(pageTitle)) return iconUrlCache.get(pageTitle);

  const url = `${WIKI_API}?action=query&titles=${encodeURIComponent(
    pageTitle
  )}&prop=pageimages&format=json&pithumbsize=64&origin=*`;

  const promise = fetch(url)
    .then((response) => (response.ok ? response.json() : null))
    .then((data) => {
      const pages = data?.query?.pages;
      const page = pages && Object.values(pages)[0];
      return page?.thumbnail?.source || null;
    })
    .catch(() => null);

  iconUrlCache.set(pageTitle, promise);
  return promise;
}
