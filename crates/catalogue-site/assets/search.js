// Driver of the search page. It runs the oxidoc-search wasm module in the
// browser and builds every result node with DOM calls. Index text only ever
// reaches the page through textContent and setAttribute.

const BASE = "/search/";
const MAX_RESULTS = 30;
const DEBOUNCE_MS = 200;
// The filter tags in URL order. The checkboxes submit under the same names.
const FILTER_KEYS = ["kind", "topic", "category", "license", "runs_on", "tier", "org"];
// Every record links an entry page. Any other link is dropped.
const RESULT_PREFIX = "/projects/";
// The slug in the path of a record, for the compare box the list script adds.
const RESULT_SLUG = /^\/projects\/([a-z0-9-]+)\/$/;
const TIER_VALUE = /^[a-z]+$/;
// The word a row prints for a fact with no value.
const NOT_MEASURED = "Not measured";
const NOT_APPLICABLE = "Not applicable";

const form = document.getElementById("search-form");
const input = document.getElementById("search-q");
const status = document.getElementById("search-status");
const results = document.getElementById("search-results");
const table = document.getElementById("search-table");
const order = document.getElementById("search-order");
const startPanel = document.getElementById("search-start");
const nonePanel = document.getElementById("search-none");
const noneQuery = document.getElementById("search-none-query");
const noneFilters = document.getElementById("search-none-filters");
const clearButton = document.getElementById("search-clear");
const noneClear = document.getElementById("search-none-clear");
const filterGroups = Array.from(form.querySelectorAll("details.filter"));
const filtersAll = document.getElementById("search-filters-all");
// Above this width the filters always show. Below it they fold behind one button.
const WIDE_BAR = window.matchMedia("(min-width: 761px)");
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
      const count = counts.get(box.name)?.get(box.value) ?? 0;
      target.textContent = `(${count})`;
      target.parentElement.classList.toggle("facet-none", count === 0);
    }
  }
}

// Writes how many boxes of each filter group are ticked, on the group's button.
function showSelected(filters) {
  for (const key of FILTER_KEYS) {
    const target = document.getElementById(`filter-selected-${key}`);
    if (target) {
      target.textContent = filters[key]?.length ? String(filters[key].length) : "";
    }
  }
  clearButton.hidden = Object.keys(filters).length === 0;
  const ticked = Object.values(filters).reduce((sum, values) => sum + values.length, 0);
  document.getElementById("filter-selected-all").textContent = ticked ? String(ticked) : "";
}

// Opens the filter bar on a wide page and folds it on a narrow one.
function fitFilterBar() {
  filtersAll.open = WIDE_BAR.matches;
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

// A span with the short text of fact `name`, and its full wording when the two differ.
function factSpan(found, name) {
  const short = found[`row_${name}`] ?? NOT_MEASURED;
  const full = found[`row_${name}_full`];
  const quiet = short === NOT_MEASURED || short === NOT_APPLICABLE
    || (full !== undefined && short.startsWith("None"));
  const span = element("span", quiet ? "fact fact-quiet" : "fact");
  if (full === undefined) {
    span.textContent = short;
    return span;
  }
  span.setAttribute("title", full);
  const seen = element("span", "", short);
  seen.setAttribute("aria-hidden", "true");
  span.append(seen, element("span", "visually-hidden", full));
  return span;
}

// A table cell for one fact, labelled for the stacked layout.
function factCell(name, label, ...content) {
  const cell = element("td", `cell-${name}`);
  cell.setAttribute("data-label", label);
  cell.append(...content);
  return cell;
}

// The cell of the latest release: the version, then its date in a time element.
function releaseCell(found) {
  const parts = [];
  if (found.row_version) {
    parts.push(element("span", "release-version", found.row_version));
  }
  if (found.row_released_iso) {
    const time = element("time", "release-date", found.row_released ?? "");
    time.setAttribute("datetime", found.row_released_iso);
    parts.push(time);
  }
  if (parts.length === 0) {
    parts.push(factSpan(found, "released"));
  }
  return factCell("release", "Latest release", ...parts);
}

// The table row of `hit`, in the markup of every other list.
function resultRow(hit) {
  const found = tagsOf(hit);
  const row = element("tr", "entry-row");
  const slug = RESULT_SLUG.exec(hit.path)?.[1];
  if (slug && typeof hit.title === "string") {
    row.setAttribute("data-slug", slug);
    row.setAttribute("data-name", hit.title);
  }
  const head = element("div", "row-head");
  const title = element("a", "row-name", hit.title);
  title.setAttribute("href", hit.path);
  head.append(title);
  if (found.kind === "app") {
    const kind = labelText("kind", found.kind);
    if (kind) {
      head.append(" ", element("span", "row-tag", kind));
    }
  }
  if (found.row_python) {
    const python = element("span", "row-tag", found.row_python);
    python.setAttribute("title", found.row_python_full ?? "");
    head.append(" ", python);
  }
  const tier = found.tier ? labelText("tier", found.tier) : null;
  if (tier && TIER_VALUE.test(found.tier)) {
    head.append(" ", element("span", `tier-badge tier-${found.tier}`, tier));
  }
  if (found.row_status) {
    const notice = element("span", "row-tag row-tag-notice", found.row_status);
    notice.setAttribute("title", found.row_status_meaning ?? "");
    head.append(" ", notice);
  }
  const project = element("th", "cell-project");
  project.setAttribute("scope", "row");
  project.append(head);
  const description = found.row_description ?? hit.snippet;
  if (description) {
    project.append(element("p", "row-description", description));
  }
  row.append(
    project,
    factCell("downloads", "Downloads", factSpan(found, "downloads")),
    factCell("dependents", "Dependents", factSpan(found, "dependents")),
    factCell("stars", "Stars", factSpan(found, "stars")),
    releaseCell(found),
    factCell("gpu", "Runs on (GPU)", factSpan(found, "gpu")),
    factCell("native", "Native code", factSpan(found, "native")),
    factCell("license", "License", factSpan(found, "license")),
  );
  return row;
}

// Replaces the result rows with `hits`, and shows the panel that fits.
function showResults(text, hits, filters) {
  const items = hits
    .filter((hit) => typeof hit.path === "string" && hit.path.startsWith(RESULT_PREFIX))
    .map(resultRow);
  results.replaceChildren(...items);
  // The list script adds its compare box to each new row.
  document.dispatchEvent(new CustomEvent("catalogue:rows"));
  const filtered = Object.keys(filters).length > 0;
  table.hidden = items.length === 0;
  order.hidden = items.length === 0;
  startPanel.hidden = Boolean(text);
  nonePanel.hidden = !text || items.length > 0;
  noneQuery.textContent = `“${text}”`;
  noneFilters.hidden = !filtered;
  // A panel says the same in words, so the status stays for screen readers only.
  status.classList.toggle("visually-hidden", items.length === 0);
  if (!text) {
    status.textContent = "Type a search term. The counts cover every entry.";
  } else if (items.length === 0) {
    status.textContent = "No entry matches.";
  } else if (items.length === MAX_RESULTS) {
    status.textContent = `The first ${MAX_RESULTS} results. Add a word or a filter to narrow them.`;
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
  showSelected(filters);
  await loadChunks(text);
  if (current !== generation) {
    return;
  }
  const hits = query(text, filters);
  const counts = facets(text, filters);
  showCounts(counts);
  showResults(text, hits, filters);
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

// Unticks every filter and runs the query again.
function clearFilters() {
  for (const box of boxes) {
    box.checked = false;
  }
  schedule(0);
}

// Closes every open filter group other than `keep`.
function closeFilters(keep) {
  for (const group of filterGroups) {
    if (group !== keep) {
      group.open = false;
    }
  }
}

readUrl();
startPanel.hidden = input.value.trim() !== "";
showSelected(checkedFilters());
fitFilterBar();
WIDE_BAR.addEventListener("change", fitFilterBar);
clearButton.addEventListener("click", clearFilters);
noneClear.addEventListener("click", clearFilters);
document.addEventListener("click", (event) => {
  closeFilters(event.target.closest?.("details.filter") ?? null);
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeFilters(null);
  }
});
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
