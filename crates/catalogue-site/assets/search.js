// Driver of the search page. It runs the oxidoc-search wasm module in the
// browser and builds every result node with DOM calls. Index text only ever
// reaches the page through textContent and setAttribute.

const BASE = "/search/";
const MAX_RESULTS = 30;
const DEBOUNCE_MS = 200;
// The filter tags in URL order. The checkboxes submit under the same names.
const FILTER_KEYS = ["kind", "topic", "category", "tier", "org"];
// Every record links an entry page. Any other link is dropped.
const RESULT_PREFIX = "/projects/";
const TIER_VALUE = /^[a-z]+$/;

const form = document.getElementById("search-form");
const input = document.getElementById("search-q");
const status = document.getElementById("search-status");
const results = document.getElementById("search-results");
const boxes = Array.from(form.querySelectorAll('input[type="checkbox"]'));

let engine = null;
const chunkLoads = new Map();
let generation = 0;
let timer = 0;

// The bytes of index file `name`.
async function fetchBytes(name) {
  const response = await fetch(BASE + name);
  if (!response.ok) {
    throw new Error(`${BASE}${name}: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

// Loads the wasm module, the metadata and the records.
async function start() {
  const module = await import(`${BASE}oxidoc_search.js`);
  await module.default();
  const [meta, records] = await Promise.all([
    fetchBytes("search-meta.bin"),
    fetchBytes("search-records.bin"),
  ]);
  module.oxidoc_search_init(meta);
  module.oxidoc_search_load_records(records);
  engine = module;
}

// Sets the query box and the checkboxes from the page URL.
function readUrl() {
  const params = new URLSearchParams(window.location.search);
  input.value = params.get("q") ?? "";
  for (const box of boxes) {
    box.checked = params.getAll(box.name).includes(box.value);
  }
}

// The checked values of each filter tag. A tag with no checked box is left out.
function checkedFilters() {
  const filters = {};
  for (const box of boxes) {
    if (box.checked) {
      (filters[box.name] ??= []).push(box.value);
    }
  }
  return filters;
}

// Writes the query and the filters to the URL without a new history entry.
function writeUrl(query, filters) {
  const params = new URLSearchParams();
  if (query) {
    params.set("q", query);
  }
  for (const key of FILTER_KEYS) {
    for (const value of filters[key] ?? []) {
      params.append(key, value);
    }
  }
  const search = params.toString();
  const url = window.location.pathname + (search ? `?${search}` : "");
  window.history.replaceState(null, "", url);
}

// Fetches and loads every chunk `query` needs that is not loaded yet.
async function loadChunks(query) {
  const needed = JSON.parse(engine.oxidoc_search_needed_chunks(query));
  await Promise.all(
    needed.map((id) => {
      if (!chunkLoads.has(id)) {
        const load = fetchBytes(`search-chunk-${id}.bin`).then((bytes) =>
          engine.oxidoc_search_load_chunk(bytes),
        );
        load.catch(() => chunkLoads.delete(id));
        chunkLoads.set(id, load);
      }
      return chunkLoads.get(id);
    }),
  );
}

// The hits of `query` under `filters`, best first.
function query(text, filters) {
  return JSON.parse(
    engine.oxidoc_search_query_filtered(text, MAX_RESULTS, JSON.stringify(filters)),
  );
}

// The facet counts of `query` under `filters`, as a map of tag to value to count.
function facets(text, filters) {
  const counts = new Map();
  const found = JSON.parse(engine.oxidoc_search_facets(text, JSON.stringify(filters)));
  for (const [key, values] of found) {
    counts.set(key, new Map(values));
  }
  return counts;
}

// The text of the label of the checkbox for tag `key` with `value`, or null.
function labelText(key, value) {
  const label = form.querySelector(`label[for="filter-${key}-${CSS.escape(value)}"]`);
  return label ? label.textContent : null;
}

// Element `tag` with class `name` and text `text`.
function element(tag, name, text) {
  const node = document.createElement(tag);
  if (name) {
    node.className = name;
  }
  if (text) {
    node.textContent = text;
  }
  return node;
}

// Writes each facet count beside its checkbox.
function showCounts(counts) {
  for (const box of boxes) {
    const target = document.getElementById(`${box.id}-count`);
    if (target) {
      target.textContent = `(${counts.get(box.name)?.get(box.value) ?? 0})`;
    }
  }
}

// The first value of each tag of `hit`, as an object of tag to value.
function tagsOf(hit) {
  const found = Object.create(null);
  for (const [key, value] of Array.isArray(hit.tags) ? hit.tags : []) {
    if (typeof key === "string" && typeof value === "string" && !(key in found)) {
      found[key] = value;
    }
  }
  return found;
}

// Replaces the result list with `hits`.
function showResults(text, hits) {
  const items = [];
  for (const hit of hits) {
    if (typeof hit.path !== "string" || !hit.path.startsWith(RESULT_PREFIX)) {
      continue;
    }
    const item = element("li", "search-result");
    const title = element("a", "search-result-title", hit.title);
    title.setAttribute("href", hit.path);
    item.append(title);
    if (hit.snippet) {
      item.append(element("p", "search-result-snippet", hit.snippet));
    }
    const found = tagsOf(hit);
    const line = element("p", "search-result-tags");
    const kind = found.kind ? labelText("kind", found.kind) : null;
    if (kind) {
      line.append(element("span", "entry-kind", kind));
    }
    const tier = found.tier ? labelText("tier", found.tier) : null;
    if (tier && TIER_VALUE.test(found.tier)) {
      line.append(" ", element("span", `tier-badge tier-${found.tier}`, tier));
    }
    if (line.childNodes.length > 0) {
      item.append(line);
    }
    items.push(item);
  }
  results.replaceChildren(...items);
  if (!text) {
    status.textContent = "Type a search term. The counts cover every entry.";
  } else if (items.length === 0) {
    status.textContent = "No entry matches.";
  } else {
    status.textContent = items.length === 1 ? "1 result." : `${items.length} results.`;
  }
}

// Runs the current query and shows its results and counts.
async function run() {
  const current = ++generation;
  const text = input.value.trim();
  const filters = checkedFilters();
  writeUrl(text, filters);
  await loadChunks(text);
  if (current !== generation) {
    return;
  }
  const hits = query(text, filters);
  const counts = facets(text, filters);
  showCounts(counts);
  showResults(text, hits);
}

// A handler that shows `message` as a plain error line in the status.
function failWith(message) {
  return (error) => {
    const topics = element("a", "", "topics");
    topics.setAttribute("href", "/topics/");
    status.replaceChildren(`${message} Browse the `, topics, " instead.");
    console.error("search:", error);
  };
}

// Runs the query after `delay` milliseconds, once the engine is ready.
function schedule(delay) {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    if (engine) {
      run().catch(failWith("The search failed."));
    }
  }, delay);
}

readUrl();
input.addEventListener("input", () => schedule(DEBOUNCE_MS));
for (const box of boxes) {
  box.addEventListener("change", () => schedule(0));
}
form.addEventListener("submit", (event) => {
  event.preventDefault();
  schedule(0);
});
status.textContent = "Loading the search index.";
start()
  .then(() => run().catch(failWith("The search failed.")))
  .catch(failWith("The search engine failed to load."));
