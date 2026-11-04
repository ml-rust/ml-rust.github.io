// The compare page. The URL is the only state it reads, so a link is shareable.
// It fetches one small file per project and builds the table with DOM calls. The
// list of names comes only when the picker needs it. Data text only ever reaches
// the page through textContent and attribute setters, and a link from a file
// reaches an href only after it passes the https check.

const DATA_DIR = "/data/compare/";
const INDEX_FILE = `${DATA_DIR}index.json`;
const NOT_MEASURED = "Not measured";
const NOT_APPLICABLE = "Not applicable";
const COMPARE_LIMIT = 4;
const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const KIND_NAMES = new Map([["library", "Library"], ["app", "App"]]);
const TIER_NAMES = new Map([
  ["featured", "Featured"],
  ["listed", "Listed"],
  ["new", "New"],
  ["archived", "Archived"],
]);
// Another script adds its copy buttons when it hears this event.
const ROWS_EVENT = "catalogue:rows";
const ONLY_DIFFERENCES = "diff";
const SOURCES_NOTE = "Each project page lists the source and date of every value.";

const status = document.getElementById("compare-status");
const notes = document.getElementById("compare-notes");
const view = document.getElementById("compare-view");

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

// `raw` as a link target, or null. The same rule as SafeHref for an outside link: https with a host.
function safeLink(raw) {
  if (typeof raw !== "string") {
    return null;
  }
  try {
    const url = new URL(raw);
    return url.protocol === "https:" && url.hostname ? url.href : null;
  } catch {
    return null;
  }
}

// A link with text `text` to `href`, which already passed a check.
function link(text, href, name) {
  const node = element("a", name, text);
  node.setAttribute("href", href);
  return node;
}

// The link to the compare page over `slugs`.
function compareHref(slugs) {
  return slugs.length ? `/compare/?p=${slugs.join(",")}` : "/compare/";
}

// The slugs in the URL, once each and in order.
function readSlugs() {
  const raw = new URLSearchParams(window.location.search).get("p") ?? "";
  const parts = raw.split(",").map((part) => part.trim()).filter(Boolean);
  return [...new Set(parts)];
}

// The value of signal `name` of `entry`, or null.
function signal(entry, name) {
  const found = Array.isArray(entry.signals)
    ? entry.signals.find((item) => item?.name === name)
    : null;
  return typeof found?.value === "string" ? found.value : null;
}

// A cell: the text the differences read, and the node the table shows.
function textCell(text) {
  if (typeof text !== "string" || text === "") {
    return { text: NOT_MEASURED, node: element("span", "fact fact-quiet", NOT_MEASURED) };
  }
  return { text, node: element("span", "fact", text) };
}

// The amount of signal `name` as a whole number, as the lists print it.
function amountCell(entry, name) {
  const value = Number.parseFloat(signal(entry, name) ?? "");
  return textCell(Number.isFinite(value) ? String(Math.round(value)) : null);
}

// The downloads of `entry` with their unit: 30 days for a library, per release for an app.
function downloadsCell(entry) {
  const app = entry.kind === "app";
  const raw = signal(entry, app ? "binary_downloads_per_release" : "downloads_per_month");
  const value = Number.parseFloat(raw ?? "");
  if (!Number.isFinite(value)) {
    return textCell(null);
  }
  const count = Math.round(value).toLocaleString("en-US");
  return textCell(app ? `${count} per release` : `${count} over 30 days`);
}

// The dependents of `entry`. An app with no crate cannot have any.
function dependentsCell(entry) {
  if (entry.kind === "app" && signal(entry, "dependents") === null) {
    return { text: NOT_APPLICABLE, node: element("span", "fact fact-quiet", NOT_APPLICABLE) };
  }
  return amountCell(entry, "dependents");
}

// `iso` as the lists print it, or null when it is no date.
function dateWords(iso) {
  const parts = typeof iso === "string" ? ISO_DATE.exec(iso) : null;
  const month = parts ? MONTHS[Number(parts[2]) - 1] : undefined;
  return month ? `${Number(parts[3])} ${month} ${parts[1]}` : null;
}

// A time element for `iso` with the text `words`.
function timeNode(iso, words) {
  const time = element("time", "", words);
  time.setAttribute("datetime", iso);
  return time;
}

// The date of signal `name` in a time element, as the lists print it.
function dateCell(entry, name) {
  const value = signal(entry, name);
  const words = dateWords(value);
  return words ? { text: value, node: timeNode(value, words) } : textCell(value);
}

// The tier as its badge. The badge prints the tier name.
function tierCell(entry) {
  const name = TIER_NAMES.get(entry.tier);
  if (!name) {
    return textCell(null);
  }
  return { text: name, node: element("span", `tier-badge tier-${entry.tier}`, name) };
}

// The one-line verdict, with the sentences of the entry page behind a closed disclosure.
// Only the verdict counts as a difference: the numbers in the details nearly always differ.
function whyCell(entry) {
  const why = entry.why;
  if (typeof why?.verdict !== "string" || why.verdict === "") {
    return textCell(null);
  }
  const box = element("div", "compare-why");
  box.append(element("p", "", why.verdict));
  const lines = (value) => (Array.isArray(value) ? value.filter((line) => typeof line === "string") : []);
  const body = element("div");
  for (const line of lines(why.lead)) {
    if (line !== why.verdict) {
      body.append(element("p", "", line));
    }
  }
  for (const [heading, key] of [
    ["What it meets", "meets"],
    ["What it lacks for Featured", "lacks"],
    ["What else the check found", "found"],
  ]) {
    const items = lines(why[key]);
    if (items.length > 0) {
      const list = element("ul");
      list.append(...items.map((line) => element("li", "", line)));
      body.append(element("h3", "", heading), list);
    }
  }
  if (body.childElementCount > 0) {
    const details = element("details", "compare-details");
    const summary = element("summary", "", "Details");
    summary.append(element("span", "visually-hidden", ` of why ${entry.name} has its tier`));
    details.append(summary, body);
    box.append(details);
  }
  return { text: why.verdict, node: box };
}

// The word of the list cell from the file, then the exact wording beneath it.
function nativeCell(entry) {
  const word = typeof entry.native_word === "string" ? entry.native_word : NOT_MEASURED;
  const exact = signal(entry, "native_code");
  const box = element("div");
  box.append(element("p", word === NOT_MEASURED ? "compare-word fact-quiet" : "compare-word", word));
  if (exact && exact !== word) {
    box.append(element("p", "compare-exact", exact));
  }
  return { text: `${word}\n${exact ?? ""}`, node: box };
}

// The install command to run. The channel page is a link in the Links row.
function installCell(entry) {
  const command = typeof entry.install?.command === "string" ? entry.install.command : null;
  if (!command) {
    return textCell(null);
  }
  const box = element("div", "compare-install");
  const line = element("p", "install-line");
  const code = element("code", "", command);
  code.setAttribute("data-install-command", command);
  line.append(code);
  box.append(line);
  return { text: command, node: box };
}

// The links of `entry`. Links always differ, so this row is never compared.
// The install channel joins them, named by its channel, unless another link already holds its page.
function linksCell(entry) {
  const list = element("ul", "compare-links");
  const shown = new Set();
  const add = (text, href) => {
    if (href && !shown.has(href)) {
      shown.add(href);
      const item = element("li");
      item.append(link(text, href));
      list.append(item);
    }
  };
  add("Project page", `/projects/${entry.slug}/`);
  add("Repository", safeLink(entry.repo));
  add("Documentation", safeLink(entry.docs));
  const channel = typeof entry.install?.text === "string" ? entry.install.text : "";
  if (channel) {
    add(channel, safeLink(entry.install?.url));
  }
  return { text: "", node: list };
}

// One row per fact, in reading order.
const FACTS = [
  { label: "Tier", cell: tierCell },
  { label: "Kind", cell: (entry) => textCell(KIND_NAMES.get(entry.kind)) },
  { label: "Description", cell: (entry) => textCell(entry.description?.text) },
  { label: "Why", cell: whyCell },
  { label: "Downloads", cell: downloadsCell },
  { label: "Dependents", cell: dependentsCell },
  { label: "Active dependents", cell: (entry) => amountCell(entry, "active_users") },
  { label: "New dependents (365 days)", cell: (entry) => amountCell(entry, "new_adopters") },
  { label: "Stars", cell: (entry) => amountCell(entry, "stars") },
  { label: "Forks", cell: (entry) => amountCell(entry, "forks") },
  { label: "Contributors", cell: (entry) => amountCell(entry, "contributors") },
  { label: "Commits", cell: (entry) => amountCell(entry, "commits") },
  { label: "Latest version", cell: (entry) => textCell(signal(entry, "latest_version")) },
  { label: "Latest release", cell: (entry) => dateCell(entry, "last_release") },
  { label: "Last human commit", cell: (entry) => dateCell(entry, "last_human_commit") },
  { label: "Runs on", cell: (entry) => textCell(signal(entry, "runs_on")) },
  { label: "Native code", cell: nativeCell },
  { label: "Python bindings", cell: (entry) => textCell(signal(entry, "python_bindings")) },
  { label: "License", cell: (entry) => textCell(signal(entry, "license")) },
  { label: "Install", cell: installCell },
  { label: "Links", cell: linksCell, plain: true },
];

// The JSON at `path`, or null when the server has no such file.
async function fetchFile(path) {
  const response = await fetch(path);
  if (response.status === 404) {
    return null;
  }
  if (!response.ok) {
    throw new Error(`${path}: HTTP ${response.status}`);
  }
  return response.json();
}

let indexLoad = null;

// Each entry of the index with a safe slug and a name. A failed fetch can run again.
function loadIndex() {
  indexLoad ??= fetchFile(INDEX_FILE).then((data) => {
    if (!Array.isArray(data?.entries)) {
      throw new Error(`${INDEX_FILE}: no entries`);
    }
    return data.entries.filter((entry) => typeof entry?.name === "string" && SLUG.test(entry?.slug ?? ""));
  });
  indexLoad.catch(() => {
    indexLoad = null;
  });
  return indexLoad;
}

// The project picker over the names of the index. It adds to the URL and follows it.
// With `eager` it fetches the index at once. Otherwise the first focus fetches it.
function picker(slugs, eager) {
  const form = element("form", "compare-picker");
  const id = "compare-add";
  const label = element("label", "", slugs.length ? "Add a project" : "Pick a project");
  label.htmlFor = id;
  const field = element("span", "search-field");
  const input = element("input");
  input.id = id;
  input.type = "search";
  input.autocomplete = "off";
  input.setAttribute("list", `${id}-names`);
  input.setAttribute("placeholder", "Type a project name");
  field.append(input);
  const names = element("datalist");
  names.id = `${id}-names`;
  const submit = element("button", "", "Add");
  submit.type = "submit";
  const error = element("p", "compare-picker-error");
  error.setAttribute("role", "alert");
  form.append(label, field, names, submit, error);
  const FAILED = "The project list failed to load. Try again, or browse the job pages.";
  const fill = () =>
    loadIndex().then((entries) => {
      const options = entries
        .filter((entry) => !slugs.includes(entry.slug))
        .map((entry) => {
          const option = element("option");
          option.value = entry.name;
          return option;
        });
      names.replaceChildren(...options);
      return entries;
    });
  const fillQuietly = () => {
    fill().catch((failure) => {
      console.error("compare:", failure);
      error.textContent = FAILED;
    });
  };
  if (eager) {
    fillQuietly();
  } else {
    input.addEventListener("focus", fillQuietly, { once: true });
  }
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const typed = input.value.trim();
    if (!typed) {
      error.textContent = "Type a project name first.";
      return;
    }
    let entries;
    try {
      entries = await fill();
    } catch (failure) {
      console.error("compare:", failure);
      error.textContent = FAILED;
      return;
    }
    const wanted = typed.toLowerCase();
    const found = entries.find((entry) => entry.name.toLowerCase() === wanted || entry.slug === wanted);
    if (found) {
      window.location.assign(compareHref([...slugs, found.slug].slice(0, COMPARE_LIMIT)));
    } else {
      error.textContent = `No project is named “${typed}”. Pick a name from the list.`;
    }
  });
  return form;
}

// The panel of a page with no project in its URL.
function emptyPanel() {
  const panel = element("section", "search-panel");
  panel.setAttribute("aria-labelledby", "compare-empty-title");
  const title = element("h2", "", "No projects picked yet");
  title.id = "compare-empty-title";
  const help = element("p", "");
  help.append(
    "On a ",
    link("job page", "/topics/"),
    " or in ",
    link("search", "/search/"),
    ", tick Compare on up to 4 rows, then follow the Compare link in the bar. Or add a project here.",
  );
  panel.append(title, help, picker([], true));
  return panel;
}

// The head of the column of `entry`, with the plain link that drops it.
function columnHead(entry, slugs) {
  const head = element("th");
  head.setAttribute("scope", "col");
  head.append(link(entry.name, `/projects/${entry.slug}/`, "compare-name"));
  const others = slugs.filter((slug) => slug !== entry.slug);
  const remove = link("Remove", compareHref(others), "compare-remove");
  remove.setAttribute("aria-label", `Remove ${entry.name}`);
  head.append(remove);
  return head;
}

// The table of `picked`, and the rows the differences toggle shows and hides.
function buildTable(picked, slugs) {
  const table = element("table", "compare-table");
  table.style.setProperty("--compare-count", String(picked.length));
  const caption = element("caption", "visually-hidden");
  caption.textContent = `Facts of ${picked.map((entry) => entry.name).join(", ")}, one column per project`;
  const headRow = element("tr");
  const corner = element("th", "", "Fact");
  corner.setAttribute("scope", "col");
  headRow.append(corner, ...picked.map((entry) => columnHead(entry, slugs)));
  const head = element("thead");
  head.append(headRow);
  const body = element("tbody");
  const same = [];
  let differ = 0;
  for (const fact of FACTS) {
    const cells = picked.map((entry) => fact.cell(entry));
    const differs = !fact.plain && new Set(cells.map((cell) => cell.text)).size > 1;
    const row = element("tr", differs ? "compare-differs" : "");
    const label = element("th", "", fact.label);
    label.setAttribute("scope", "row");
    if (differs) {
      differ += 1;
      label.append(" ", element("span", "row-tag", "Differs"));
    } else if (!fact.plain) {
      same.push(row);
    }
    row.append(label);
    for (const cell of cells) {
      const data = element("td");
      data.append(cell.node);
      row.append(data);
    }
    body.append(row);
  }
  table.append(caption, head, body);
  return { table, same, differ };
}

// The whole view for `picked`.
function showTable(picked, slugs) {
  const { table, same, differ } = buildTable(picked, slugs);
  const scroller = element("div", "compare-scroll");
  scroller.tabIndex = 0;
  scroller.setAttribute("role", "region");
  scroller.setAttribute("aria-label", "Comparison table. It scrolls sideways.");
  scroller.append(table);
  const total = FACTS.filter((fact) => !fact.plain).length;
  const summary = picked.length > 1
    ? `${picked.length} projects. ${differ} of ${total} facts differ.`
    : "1 project. Add another to see what differs.";
  status.textContent = summary;
  const bar = element("div", "compare-bar");
  if (picked.length > 1) {
    const toggle = element("label", "filter-check");
    const box = element("input");
    box.type = "checkbox";
    const params = new URLSearchParams(window.location.search);
    box.checked = params.get(ONLY_DIFFERENCES) === "1";
    toggle.append(box, "Show only differences");
    const apply = () => {
      for (const row of same) {
        row.hidden = box.checked;
      }
      const url = new URLSearchParams(window.location.search);
      if (box.checked) {
        url.set(ONLY_DIFFERENCES, "1");
      } else {
        url.delete(ONLY_DIFFERENCES);
      }
      const search = url.toString().replaceAll("%2C", ",");
      window.history.replaceState(null, "", window.location.pathname + (search ? `?${search}` : ""));
      status.textContent = box.checked
        ? `${summary} The ${same.length} rows that match are hidden.`
        : summary;
    };
    box.addEventListener("change", apply);
    bar.append(toggle);
    view.append(bar, scroller);
    apply();
  } else {
    view.append(scroller);
  }
  view.append(element("p", "compare-note", SOURCES_NOTE));
  if (picked.length < COMPARE_LIMIT) {
    view.append(picker(slugs, false));
  }
  document.dispatchEvent(new CustomEvent(ROWS_EVENT));
}

// Names each slug the page cannot show.
function showNotes(unknown, failed, extra) {
  const lines = [
    ...unknown.map((slug) => `No project has the id “${slug}”. The table skips it.`),
    ...failed.map((slug) => `The file of “${slug}” failed to load. The table skips it.`),
    ...extra.map((slug) => `The table holds ${COMPARE_LIMIT} projects, so it skips “${slug}”.`),
  ];
  notes.replaceChildren(...lines.map((line) => element("li", "", line)));
  notes.hidden = lines.length === 0;
}

// The outcome of fetching the file of `slug`. A missing file, or a file about another project,
// is an unknown slug. A file that does not load or parse is a failure.
async function loadEntry(slug) {
  try {
    const data = await fetchFile(`${DATA_DIR}${slug}.json`);
    const matches = data?.slug === slug && typeof data?.name === "string";
    return matches ? { slug, entry: data } : { slug, unknown: true };
  } catch (failure) {
    console.error("compare:", failure);
    return { slug, failed: true };
  }
}

// Fetches the files the URL asks for and shows the page. Only a slug of the safe shape
// reaches a file path.
async function start() {
  const asked = readSlugs();
  const safe = asked.filter((slug) => SLUG.test(slug));
  const wanted = safe.slice(0, COMPARE_LIMIT);
  if (wanted.length > 0) {
    status.textContent = "Loading the project files.";
  }
  const found = await Promise.all(wanted.map(loadEntry));
  const picked = found.filter((item) => item.entry);
  const slugs = picked.map((item) => item.slug);
  showNotes(
    [...asked.filter((slug) => !SLUG.test(slug)), ...found.filter((item) => item.unknown).map((item) => item.slug)],
    found.filter((item) => item.failed).map((item) => item.slug),
    safe.slice(COMPARE_LIMIT),
  );
  if (picked.length === 0) {
    status.textContent = "";
    view.replaceChildren(emptyPanel());
    return;
  }
  showTable(picked.map((item) => item.entry), slugs);
}

start().catch((error) => {
  console.error("compare:", error);
  const topics = link("job pages", "/topics/");
  status.replaceChildren("The comparison failed to load. The ", topics, " list the same facts.");
});
