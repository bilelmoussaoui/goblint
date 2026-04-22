const ruleList = document.getElementById("ruleList");
const ruleDetail = document.getElementById("ruleDetail");
const searchInput = document.getElementById("search");
const categoryFilter = document.getElementById("categoryFilter");

let rules = [];
let filteredRules = [];

function formatCategory(category) {
  const categoryMap = {
    correctness: "Correctness",
    suspicious: "Suspicious",
    style: "Style",
    complexity: "Complexity",
    perf: "Performance",
    pedantic: "Pedantic",
    restriction: "Restriction",
    portability: "Portability",
  };
  return categoryMap[category] || category;
}

function badge(text, colorClass) {
  return `<span class="px-2.5 py-1 text-sm rounded border ${colorClass}">${text}</span>`;
}

function renderList() {
  // Sort rules alphabetically by name
  const sortedRules = [...filteredRules].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  ruleList.innerHTML = sortedRules
    .map(
      (r) => `
    <div onclick="selectRule('${r.name}')" class="rule-item px-4 py-3 border-b cursor-pointer">
      <div class="rule-item-name" title="${r.name}">${r.name}</div>
      <div class="text-sm rule-item-category mt-1">${formatCategory(r.category)}</div>
    </div>
  `,
    )
    .join("");
}

function renderDetail(rule) {
  const configSection =
    rule.config_options && rule.config_options.length > 0
      ? `
    <div>
      <h3 class="text-lg font-semibold detail-label mb-3">Configuration</h3>
      ${rule.config_options
        .map(
          (opt) => `
        <div class="mb-4 code-block p-4 rounded-lg border">
          <div class="flex items-baseline gap-2 mb-1">
            <code class="detail-text font-semibold">${opt.name}</code>
            <span class="text-sm detail-label">(${opt.option_type})</span>
          </div>
          <div class="text-sm detail-text mb-1">${opt.description}</div>
          <div class="text-sm detail-label">Default: <code class="detail-text">${opt.default_value}</code></div>
        </div>
      `,
        )
        .join("")}
      <div class="text-sm detail-label mt-3">
        Example configuration in <code>goblint.toml</code>:
        <pre class="code-block p-3 rounded-lg border mt-2 text-xs">[rules.${rule.name}]
${rule.config_options.map((opt) => `${opt.name} = ${opt.example_value}`).join("\n")}</pre>
      </div>
    </div>
  `
      : "";

  ruleDetail.innerHTML = `
    <h2 class="text-3xl font-bold rule-title mb-4">${rule.name}</h2>

    <div class="flex gap-2 mb-6 flex-wrap">
      ${badge(formatCategory(rule.category), "badge")}
      ${rule.fixable ? badge("Fixable", "badge-green") : ""}
      ${rule.requires_auto_cleanup ? badge("Not MSVC compatible", "badge-orange") : ""}
      ${rule.min_glib_version !== "2.0" ? badge(`GLib ${rule.min_glib_version}+`, "badge-purple") : ""}
    </div>

    <div class="border-t divider pt-6 space-y-6">
      <div>
        <p class="detail-text text-base">${rule.description || "No description yet."}</p>
      </div>

      ${
        rule.long_description
          ? `
        <div class="prose-custom">
          ${marked.parse(rule.long_description)}
        </div>
      `
          : ""
      }

      ${configSection}
    </div>
  `;
}

function selectRule(name) {
  const rule = rules.find((r) => r.name === name);
  if (!rule) return;
  renderDetail(rule);
  location.hash = name;
}

function applyFilters() {
  const query = searchInput.value.toLowerCase();
  const category = categoryFilter.value;

  filteredRules = rules.filter((r) => {
    return (
      (!query || r.name.toLowerCase().includes(query)) &&
      (!category || r.category === category)
    );
  });

  renderList();
}

function populateFilters() {
  const categories = [...new Set(rules.map((r) => r.category))].sort();
  categories.forEach((cat) => {
    const opt = document.createElement("option");
    opt.value = cat;
    opt.textContent = formatCategory(cat);
    categoryFilter.appendChild(opt);
  });
}

fetch("rules.json")
  .then((res) => res.json())
  .then((data) => {
    rules = data.rules;
    filteredRules = data.rules;
    populateFilters();
    renderList();

    const hash = decodeURIComponent(location.hash.slice(1));
    if (hash) {
      selectRule(hash);
    } else if (rules.length > 0) {
      // Auto-select first rule alphabetically
      const sortedRules = [...rules].sort((a, b) => a.name.localeCompare(b.name));
      selectRule(sortedRules[0].name);
    }
  });

searchInput.addEventListener("input", applyFilters);
categoryFilter.addEventListener("change", applyFilters);
