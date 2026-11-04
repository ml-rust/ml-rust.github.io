// The list pages: sort, filter, copy and the compare selection. Each part works
// on what the page holds and adds its own controls, so a page with no script
// shows no dead control. Text only ever reaches the page through textContent
// and attribute setters. A fact comes from a data attribute of its row, never
// from the text of a cell.

const NOT_MEASURED = "Not measured";
const NOT_CHECKED = "Not checked";
const NONE_STATED = "None stated";
const NONE_FOUND = "None found";
// The most projects the compare page shows.
const COMPARE_LIMIT = 4;
const COMPARE_STORE = "catalogue-compare";
const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/;
// Another script sends this event after it adds rows or install commands.
const ROWS_EVENT = "catalogue:rows";
const COPY_NOTE_MS = 2500;
const WIDE_BAR = window.matchMedia("(min-width: 761px)");

// Each sort: its URL key, column, row attribute, first direction and words.
const SORTS = [
  { key: "name", column: ".col-project", attr: "name", first: "asc", hidden: " name",
    label: "Project", words: { asc: "A to Z", desc: "Z to A" } },
  { key: "downloads", column: ".col-downloads", attr: "downloads", first: "desc", numeric: true,
    label: "Downloads", words: { desc: "most first", asc: "fewest first" } },
  { key: "dependents", column: ".col-dependents", attr: "dependents", first: "desc", numeric: true,
    label: "Dependents", words: { desc: "most first", asc: "fewest first" } },
  { key: "stars", column: ".col-stars", attr: "stars", first: "desc", numeric: true,
    label: "Stars", words: { desc: "most first", asc: "fewest first" } },
  { key: "released", column: ".col-release", attr: "released", first: "desc",
    label: "Latest release", words: { desc: "newest first", asc: "oldest first" } },
];
const ARIA_SORT = { asc: "ascending", desc: "descending" };

// Each filter: its URL key, heading, row attribute and fixed option order.
// A row with no value reads as `missing`. An answer of its own, never a yes or a no.
const FILTERS = [
  { key: "kind", legend: "Kind", attr: "kindName" },
  { key: "tier", legend: "Tier", attr: "tierName", missing: NOT_CHECKED,
    order: ["Featured", "Listed", "New", "Archived"] },
  { key: "gpu", legend: "GPU backend", attr: "gpu" },
  { key: "native", legend: "Native code", attr: "native",
    order: [NONE_FOUND, "Optional", "Required", "Build tool"] },
  { key: "license", legend: "License family", attr: "licenseFamily",
    order: ["Permissive", "Weak copyleft", "Strong copyleft", "Other"] },
  { key: "python", legend: "Python bindings", attr: "python",
    order: ["Python bindings", NONE_FOUND] },
];
const HIDE_DORMANT = "hide_dormant";
// These answers print after every named value.
const LAST_OPTIONS = [NONE_FOUND, NONE_STATED, NOT_CHECKED, NOT_MEASURED];

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

// A plain button with class `name` and text `text`.
function button(name, text) {
  const node = element("button", name, text);
  node.type = "button";
  return node;
}

// `count` projects in words.
function projectCount(count) {
  return count === 1 ? "1 project" : `${count} projects`;
}

// How many projects `row` stands for.
function weight(row) {
  return Number(row.dataset.projects) || 1;
}

// The sum of the projects of `rows`.
function projectsIn(rows) {
  return rows.reduce((sum, row) => sum + weight(row), 0);
}

// Writes `change` to the query string with no new history entry. It keeps every other key.
function writeQuery(change) {
  const params = new URLSearchParams(window.location.search);
  change(params);
  const search = params.toString();
  const url = window.location.pathname + (search ? `?${search}` : "") + window.location.hash;
  window.history.replaceState(null, "", url);
}

// The chevron of a filter button, the same drawing as the search page.
function chevron() {
  const space = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(space, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.75");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  const path = document.createElementNS(space, "path");
  path.setAttribute("d", "M5.5 9l6.5 6.5L18.5 9");
  svg.append(path);
  const icon = element("span", "icon");
  icon.setAttribute("aria-hidden", "true");
  icon.append(svg);
  return icon;
}

// One list table: its rows, its tier groups, and the sort and filter state.
class ListTable {
  constructor(table) {
    this.table = table;
    this.mixed = table.classList.contains("rows-mixed");
    this.groups = Array.from(table.tBodies).map((body) => ({
      body,
      rows: Array.from(body.querySelectorAll("tr.entry-row")),
      count: body.querySelector(".rows-group-count"),
    }));
    this.rows = this.groups.flatMap((group) => group.rows);
    this.sorted = element("tbody");
    this.sorted.hidden = true;
    table.append(this.sorted);
    this.sort = null;
    this.selected = new Map();
    this.hideDormant = false;
    this.filters = [];
    this.note = (table.closest("section") ?? document).querySelector(".order-note");
    this.defaultNote = this.note ? this.note.textContent : "";
    this.jumps = document.querySelector(".tier-jumps");
    this.total = projectsIn(this.rows);
  }

  // Adds a sort button to each sortable column head.
  addSortButtons() {
    this.buttons = new Map();
    for (const sort of SORTS) {
      const head = this.table.tHead?.querySelector(sort.column);
      if (!head) {
        continue;
      }
      const control = button("sort-button");
      control.append(head.textContent);
      if (sort.hidden) {
        control.append(element("span", "visually-hidden", sort.hidden));
      }
      const mark = element("span", "sort-mark");
      mark.setAttribute("aria-hidden", "true");
      control.append(mark);
      head.replaceChildren(control);
      head.classList.add("col-sortable");
      control.addEventListener("click", () => this.pressSort(sort));
      this.buttons.set(sort.key, { head, control, mark });
    }
    if (this.note) {
      this.note.setAttribute("aria-live", "polite");
    }
  }

  // Sets the sort: `sort` in its first direction, or flipped when it is the current one.
  pressSort(sort) {
    const flip = this.sort?.key === sort.key;
    const dir = flip ? (this.sort.dir === "asc" ? "desc" : "asc") : sort.first;
    this.sort = { key: sort.key, dir };
    this.apply();
  }

  // Compares two rows under the current sort. A missing value comes last in both directions.
  compare(a, b) {
    const sort = SORTS.find((item) => item.key === this.sort.key);
    const left = a.dataset[sort.attr];
    const right = b.dataset[sort.attr];
    let order = 0;
    if (left === undefined || right === undefined) {
      order = Number(left === undefined) - Number(right === undefined);
    } else {
      const raw = sort.numeric
        ? Number(left) - Number(right)
        : left.toLowerCase().localeCompare(right.toLowerCase());
      order = this.sort.dir === "asc" ? raw : -raw;
    }
    if (order !== 0) {
      return order;
    }
    const names = (a.dataset.name ?? "").toLowerCase().localeCompare((b.dataset.name ?? "").toLowerCase());
    return names !== 0 ? names : (a.dataset.slug ?? "").localeCompare(b.dataset.slug ?? "");
  }

  // Puts the rows in the sorted body, or back in their tier groups in the published order.
  placeRows() {
    const flat = this.sort !== null;
    if (flat) {
      this.sorted.append(...[...this.rows].sort((a, b) => this.compare(a, b)));
    } else {
      for (const group of this.groups) {
        group.body.append(...group.rows);
      }
    }
    this.sorted.hidden = !flat;
    // A flat list has no group heading, so each row prints its own tier.
    this.table.classList.toggle("rows-mixed", this.mixed || flat);
    if (this.jumps) {
      this.jumps.hidden = flat;
    }
  }

  // Writes the current order on each head, in `aria-sort` and in the order note.
  showSort() {
    for (const sort of SORTS) {
      const found = this.buttons.get(sort.key);
      if (!found) {
        continue;
      }
      if (this.sort?.key === sort.key) {
        found.head.setAttribute("aria-sort", ARIA_SORT[this.sort.dir]);
        found.mark.textContent = this.sort.dir === "asc" ? "↑" : "↓";
      } else {
        found.head.removeAttribute("aria-sort");
        found.mark.textContent = "↕";
      }
    }
    if (!this.note) {
      return;
    }
    if (!this.sort) {
      this.note.replaceChildren(this.defaultNote);
      return;
    }
    const sort = SORTS.find((item) => item.key === this.sort.key);
    const back = button("link-button order-back", "Back to the published order");
    back.addEventListener("click", () => {
      this.sort = null;
      this.apply();
      this.buttons.get(sort.key)?.control.focus();
    });
    this.note.replaceChildren(
      `Your sort: ${sort.label}, ${sort.words[this.sort.dir]}. Missing values stay last. `,
      back,
    );
  }

  // The values of filter `filter` that `row` holds.
  valuesOf(row, filter) {
    const value = row.dataset[filter.attr];
    return value === undefined ? [filter.missing ?? NOT_MEASURED] : value.split("|");
  }

  // Whether `row` is hidden as dormant.
  hiddenDormant(row) {
    return this.hideDormant && row.dataset.dormancy === "dormant";
  }

  // Whether `row` passes the dormant choice and every filter other than `skip`.
  passes(row, skip) {
    return !this.hiddenDormant(row) && this.passesFilters(row, skip);
  }

  // Whether `row` passes every filter other than `skip`.
  passesFilters(row, skip) {
    return this.filters.every((filter) => {
      const chosen = this.selected.get(filter.key);
      return filter === skip || !chosen?.size || this.valuesOf(row, filter).some((value) => chosen.has(value));
    });
  }

  // Every option of `filter` on this page, in display order.
  optionsOf(filter) {
    const seen = new Set(this.rows.flatMap((row) => this.valuesOf(row, filter)));
    const place = (value) => {
      const last = LAST_OPTIONS.indexOf(value);
      if (last >= 0 && !filter.order?.includes(value)) {
        return 1000 + last;
      }
      const fixed = filter.order?.indexOf(value) ?? -1;
      return fixed >= 0 ? fixed : 500;
    };
    return [...seen].sort((a, b) => place(a) - place(b) || a.localeCompare(b));
  }

  // Builds the filter bar above the table. A filter with one option stays out.
  addFilterBar() {
    this.filters = FILTERS.filter((filter) => this.optionsOf(filter).length > 1);
    const dormant = projectsIn(this.rows.filter((row) => row.dataset.dormancy === "dormant"));
    const canHide = dormant > 0 && dormant < this.total;
    if (this.filters.length === 0 && !canHide) {
      return;
    }
    const bar = element("div", "search-filters");
    bar.append(element("span", "filters-label", "Filter"));
    this.boxes = [];
    for (const filter of this.filters) {
      bar.append(this.filterGroup(filter));
    }
    if (canHide) {
      const label = element("label", "filter-check");
      this.hideBox = element("input");
      this.hideBox.type = "checkbox";
      this.hideCount = element("span", "facet-count");
      label.append(this.hideBox, "Hide dormant ", this.hideCount);
      this.hideBox.addEventListener("change", () => {
        this.hideDormant = this.hideBox.checked;
        this.apply();
      });
      bar.append(label);
    }
    const fold = element("details", "filters-all");
    const toggle = element("summary", "filters-toggle", "Filters");
    this.selectedCount = element("span", "filter-selected");
    toggle.append(this.selectedCount, chevron());
    fold.append(toggle, bar);
    const fit = () => {
      fold.open = WIDE_BAR.matches;
    };
    fit();
    WIDE_BAR.addEventListener("change", fit);

    this.status = element("div", "list-status");
    this.status.setAttribute("role", "status");
    this.status.tabIndex = -1;
    this.statusText = element("span");
    this.clear = button("filter-clear", "Clear filters");
    this.clear.addEventListener("click", () => this.clearFilters());
    this.status.append(this.statusText, " ", this.clear);

    const wrap = element("div", "list-filters");
    wrap.append(fold, this.status);
    this.table.before(wrap);
    this.closeOutside(wrap);
  }

  // One dropdown of checkboxes, in the markup of the search page.
  filterGroup(filter) {
    const group = element("details", "filter");
    group.setAttribute("name", "list-filters");
    const summary = element("summary", "", filter.legend);
    filter.count = element("span", "filter-selected");
    summary.append(filter.count, chevron());
    const set = element("fieldset", "search-filter");
    set.append(element("legend", "", filter.legend));
    const list = element("ul");
    filter.options = this.optionsOf(filter).map((value, index) => {
      const item = element("li");
      const box = element("input");
      box.type = "checkbox";
      box.id = `list-filter-${filter.key}-${index}`;
      const label = element("label", "", value);
      label.htmlFor = box.id;
      const count = element("span", "facet-count");
      item.append(box, " ", label, " ", count);
      list.append(item);
      box.addEventListener("change", () => {
        const chosen = this.selected.get(filter.key) ?? new Set();
        if (box.checked) {
          chosen.add(value);
        } else {
          chosen.delete(value);
        }
        this.selected.set(filter.key, chosen);
        this.apply();
      });
      return { value, box, count, item };
    });
    set.append(list);
    group.append(summary, set);
    return group;
  }

  // Closes an open dropdown on a click elsewhere or on Escape.
  closeOutside(wrap) {
    const groups = Array.from(wrap.querySelectorAll("details.filter"));
    const close = (keep) => {
      for (const group of groups) {
        if (group !== keep) {
          group.open = false;
        }
      }
    };
    document.addEventListener("click", (event) => {
      close(event.target.closest?.("details.filter") ?? null);
    });
    document.addEventListener("keydown", (event) => {
      if (event.key !== "Escape") {
        return;
      }
      const open = groups.find((group) => group.open);
      close(null);
      open?.querySelector("summary").focus();
    });
  }

  // Drops every filter and moves the focus off the button that hides itself.
  clearFilters() {
    this.selected.clear();
    this.hideDormant = false;
    this.apply();
    this.status.focus();
  }

  // Whether any filter is on.
  filtering() {
    return this.hideDormant || [...this.selected.values()].some((chosen) => chosen.size > 0);
  }

  // Shows the rows that pass, with every count, the group headings and the status.
  showFilters() {
    for (const row of this.rows) {
      row.hidden = !this.passes(row, null);
    }
    const visible = this.rows.filter((row) => !row.hidden);
    const shown = projectsIn(visible);
    for (const group of this.groups) {
      const left = group.rows.filter((row) => !row.hidden);
      if (group.count) {
        group.body.hidden = this.sort !== null || left.length === 0;
        group.count.textContent = projectCount(projectsIn(left));
        this.showJump(group, left);
      } else {
        group.body.hidden = this.sort !== null;
      }
    }
    this.table.hidden = visible.length === 0;
    if (!this.status) {
      return;
    }
    let ticked = this.hideDormant ? 1 : 0;
    for (const filter of this.filters) {
      const chosen = this.selected.get(filter.key);
      const count = chosen?.size ?? 0;
      ticked += count;
      filter.count.textContent = count ? String(count) : "";
      const open = this.rows.filter((row) => this.passes(row, filter));
      for (const option of filter.options) {
        option.box.checked = chosen?.has(option.value) ?? false;
        const found = open.filter((row) => this.valuesOf(row, filter).includes(option.value));
        const projects = projectsIn(found);
        option.count.textContent = `(${projects})`;
        option.item.classList.toggle("facet-none", projects === 0);
      }
    }
    if (this.hideBox) {
      this.hideBox.checked = this.hideDormant;
      const dormant = this.rows.filter(
        (row) => row.dataset.dormancy === "dormant" && this.passesFilters(row, null),
      );
      this.hideCount.textContent = `(${projectsIn(dormant)})`;
    }
    this.selectedCount.textContent = ticked ? String(ticked) : "";
    const none = visible.length === 0 ? " No project matches these filters." : "";
    this.statusText.textContent = `${shown} of ${this.total} projects.${none}`;
    this.clear.hidden = !this.filtering();
  }

  // Writes the count of `group` on its tier link, and hides the link of an empty group.
  showJump(group, left) {
    const name = Array.from(group.body.classList).find((item) => item.startsWith("rows-group-"));
    const link = name && this.jumps?.querySelector(`a[href="#${name.slice("rows-group-".length)}"]`);
    if (!link) {
      return;
    }
    link.parentElement.hidden = left.length === 0;
    const count = link.querySelector(".tier-jump-count");
    if (count) {
      count.textContent = String(projectsIn(left));
    }
  }

  // Sets the sort and the filters from the query string. An unknown value is dropped.
  readUrl() {
    const params = new URLSearchParams(window.location.search);
    const [key, dir] = (params.get("sort") ?? "").split("-");
    if (this.buttons?.has(key) && (dir === "asc" || dir === "desc")) {
      this.sort = { key, dir };
    }
    for (const filter of this.filters) {
      const known = new Set(filter.options.map((option) => option.value));
      this.selected.set(filter.key, new Set(params.getAll(filter.key).filter((value) => known.has(value))));
    }
    this.hideDormant = Boolean(this.hideBox) && params.get(HIDE_DORMANT) === "1";
  }

  // Writes the sort and the filters to the query string.
  writeUrl() {
    writeQuery((params) => {
      params.delete("sort");
      params.delete(HIDE_DORMANT);
      for (const filter of this.filters) {
        params.delete(filter.key);
        for (const value of this.selected.get(filter.key) ?? []) {
          params.append(filter.key, value);
        }
      }
      if (this.sort) {
        params.set("sort", `${this.sort.key}-${this.sort.dir}`);
      }
      if (this.hideDormant) {
        params.set(HIDE_DORMANT, "1");
      }
    });
  }

  // Brings the page in line with the state.
  apply() {
    this.placeRows();
    this.showSort();
    this.showFilters();
    this.writeUrl();
  }
}

// The compare selection, kept for this tab across pages.
class CompareSelection {
  constructor() {
    this.items = this.read();
    this.tray = null;
  }

  // The stored selection. A value of any other shape is dropped.
  read() {
    try {
      const found = JSON.parse(window.sessionStorage.getItem(COMPARE_STORE) ?? "[]");
      return Array.isArray(found)
        ? found
            .filter((item) => typeof item?.name === "string" && SLUG.test(item?.slug ?? ""))
            .slice(0, COMPARE_LIMIT)
        : [];
    } catch {
      return [];
    }
  }

  // Stores the selection. A browser that refuses keeps it for this page only.
  write() {
    try {
      window.sessionStorage.setItem(COMPARE_STORE, JSON.stringify(this.items));
    } catch {
      console.warn(`compare: ${COMPARE_STORE}: storage refused. The selection lasts for this page.`);
    }
  }

  // Adds a box to each row that names its project, and shows the bar.
  addBoxes() {
    const rows = Array.from(document.querySelectorAll("tr.entry-row[data-slug][data-name]"));
    if (rows.length === 0 && !this.tray) {
      return;
    }
    for (const row of rows) {
      if (!row.querySelector(".row-compare")) {
        this.addBox(row);
      }
    }
    this.show();
  }

  // The box of `row`, with the label `Compare <name>`.
  addBox(row) {
    const { slug, name } = row.dataset;
    const head = row.querySelector(".row-head");
    if (!head || !SLUG.test(slug)) {
      return;
    }
    const label = element("label", "row-compare");
    const box = element("input");
    box.type = "checkbox";
    box.dataset.compareSlug = slug;
    label.append(box, element("span", "", "Compare"), element("span", "visually-hidden", ` ${name}`));
    box.addEventListener("change", () => {
      if (box.checked) {
        this.add(slug, name);
      } else {
        this.remove(slug);
      }
    });
    head.append(label);
  }

  add(slug, name) {
    if (this.items.length < COMPARE_LIMIT && !this.items.some((item) => item.slug === slug)) {
      this.items.push({ slug, name });
      this.changed(`${name} added.`);
    }
  }

  remove(slug) {
    const found = this.items.find((item) => item.slug === slug);
    this.items = this.items.filter((item) => item.slug !== slug);
    this.changed(found ? `${found.name} removed.` : "");
  }

  changed(words) {
    this.write();
    this.show();
    this.live.textContent = `${words} ${this.items.length} of ${COMPARE_LIMIT} selected.`;
  }

  // Creates the bar once, before the footer.
  createTray() {
    this.tray = element("aside", "compare-tray");
    this.tray.setAttribute("aria-label", "Projects to compare");
    this.names = element("ul", "compare-tray-names");
    this.count = element("span", "compare-tray-count");
    this.link = element("a", "button button-primary", "Compare");
    const clear = button("link-button", "Clear");
    clear.addEventListener("click", () => {
      this.items = [];
      this.changed("Selection cleared.");
      document.getElementById("main")?.focus();
    });
    this.live = element("p", "visually-hidden");
    this.live.setAttribute("aria-live", "polite");
    const actions = element("div", "compare-tray-actions");
    actions.append(this.count, clear, this.link);
    const inner = element("div", "compare-tray-inner");
    inner.append(this.names, actions);
    this.tray.append(inner);
    (document.querySelector(".site-footer") ?? document.body).before(this.live, this.tray);
  }

  // Brings the bar and every box in line with the selection.
  show() {
    if (!this.tray) {
      this.createTray();
    }
    const full = this.items.length >= COMPARE_LIMIT;
    for (const box of document.querySelectorAll("input[data-compare-slug]")) {
      box.checked = this.items.some((item) => item.slug === box.dataset.compareSlug);
      box.disabled = full && !box.checked;
    }
    this.tray.hidden = this.items.length === 0;
    document.documentElement.classList.toggle("has-compare-tray", this.items.length > 0);
    this.names.replaceChildren(...this.items.map((item, index) => this.nameItem(item, index)));
    this.count.textContent = full
      ? `${COMPARE_LIMIT} of ${COMPARE_LIMIT}. Remove one to add another.`
      : `${this.items.length} of ${COMPARE_LIMIT}`;
    this.link.setAttribute("href", `/compare/?p=${this.items.map((item) => item.slug).join(",")}`);
  }

  // One selected name with its remove button.
  nameItem(item, index) {
    const entry = element("li", "compare-tray-name");
    const remove = button("compare-tray-remove", "×");
    remove.setAttribute("aria-label", `Remove ${item.name}`);
    remove.addEventListener("click", () => {
      this.remove(item.slug);
      const next = this.names.querySelectorAll("button")[index] ?? this.link;
      if (this.items.length > 0) {
        next.focus();
      } else {
        document.getElementById("main")?.focus();
      }
    });
    entry.append(element("span", "", item.name), remove);
    return entry;
  }
}

// Adds a copy button to each install command that has none.
function addCopyButtons() {
  if (!navigator.clipboard?.writeText) {
    return;
  }
  for (const code of document.querySelectorAll("code[data-install-command]")) {
    if (code.nextElementSibling?.classList.contains("copy-button")) {
      continue;
    }
    const command = code.dataset.installCommand;
    const copy = button("copy-button", "Copy");
    copy.setAttribute("aria-label", `Copy ${command}`);
    const note = element("span", "visually-hidden");
    note.setAttribute("aria-live", "polite");
    let timer = 0;
    copy.addEventListener("click", () => {
      window.clearTimeout(timer);
      navigator.clipboard.writeText(command).then(
        () => {
          // The button says it in place, so nothing around it moves.
          copy.textContent = "Copied";
          note.textContent = "Copied";
          timer = window.setTimeout(() => {
            copy.textContent = "Copy";
            note.textContent = "";
          }, COPY_NOTE_MS);
        },
        () => {
          note.className = "copy-note";
          note.textContent = "Copy failed. Select the text instead.";
        },
      );
    });
    code.after(copy, note);
  }
}

const selection = new CompareSelection();
const tableNode = document.querySelector("table[data-sortable]");
if (tableNode) {
  const list = new ListTable(tableNode);
  list.addSortButtons();
  if (tableNode.hasAttribute("data-filterable")) {
    list.addFilterBar();
  }
  list.readUrl();
  list.apply();
}
selection.addBoxes();
addCopyButtons();
document.addEventListener(ROWS_EVENT, () => {
  selection.addBoxes();
  addCopyButtons();
});
