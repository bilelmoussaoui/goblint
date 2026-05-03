const ruleList = document.getElementById("ruleList");
const ruleDetail = document.getElementById("ruleDetail");
const searchInput = document.getElementById("search");
const categoryFilter = document.getElementById("categoryFilter");
const mobileMenuBtn = document.getElementById("mobileMenuBtn");
const mobileOverlay = document.getElementById("mobileOverlay");
const sidebar = document.getElementById("sidebar");

let rules = [];
let filteredRules = [];
let selectedRuleName = null;

// Mobile menu toggle
function toggleMobileMenu() {
  sidebar.classList.toggle("mobile-open");
  mobileOverlay.classList.toggle("active");
}

function closeMobileMenu() {
  sidebar.classList.remove("mobile-open");
  mobileOverlay.classList.remove("active");
}

mobileMenuBtn.addEventListener("click", toggleMobileMenu);
mobileOverlay.addEventListener("click", closeMobileMenu);

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

function formatType(type) {
  return type.replace(/</g, "[").replace(/>/g, "]");
}

function renderList() {
  // Sort rules alphabetically by name
  const sortedRules = [...filteredRules].sort((a, b) =>
    a.name.localeCompare(b.name),
  );

  ruleList.innerHTML = sortedRules
    .map((r) => {
      const isActive = r.name === selectedRuleName;
      const activeClass = isActive ? " active" : "";
      return `
    <div onclick="selectRule('${r.name}')" class="rule-item${activeClass} px-4 py-3 border-b cursor-pointer">
      <div class="rule-item-name" title="${r.name}">${r.name}</div>
      <div class="text-sm rule-item-category mt-1">${formatCategory(r.category)}</div>
    </div>
  `;
    })
    .join("");
}

function renderDetail(rule) {
  const configSection =
    rule.config_options && rule.config_options.length > 0
      ? `
    <div>
      <h3 class="text-lg font-semibold detail-label mb-4">Configuration</h3>
      <div class="config-grid mb-6">
        ${rule.config_options
          .map(
            (opt) => `
          <div class="config-option">
            <div class="config-option-header">
              <code class="config-option-name">${opt.name}</code>
              <span class="config-option-type">${formatType(opt.option_type)}</span>
            </div>
            <p class="config-option-desc">${opt.description}</p>
            <div class="config-option-default">
              Default: <code>${opt.default_value}</code>
            </div>
          </div>
        `,
          )
          .join("")}
      </div>
      <div class="config-example">
        <div class="config-example-header">
          <span class="config-example-title">Example Configuration</span>
          <span class="config-example-file">goblint.toml</span>
        </div>
        <pre class="config-example-code">[rules.${rule.name}]
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
      ${rule.opt_in ? badge("Opt-in", "badge-yellow") : ""}
      ${rule.requires_meson ? badge("Meson", "badge-meson") : ""}
      ${rule.requires_auto_cleanup ? badge("Not MSVC compatible", "badge-orange") : ""}
      ${rule.min_glib_version !== "2.0" ? badge(`GLib ${rule.min_glib_version}+`, "badge-purple") : ""}
    </div>

    ${rule.opt_in ? `
    <div class="opt-in-notice mb-6">
      <strong>Disabled by default.</strong> This rule may produce false positives due to
      fundamental limitations of static analysis without a preprocessor or full call graph.
      Enable it explicitly in your config or with <code>--only ${rule.name}</code>.
    </div>` : ""}
    ${rule.requires_meson ? `
    <div class="meson-notice mb-6">
      <strong>Requires Meson introspection.</strong> This rule only runs when a Meson build
      directory is available. Without it, results will be silently skipped.
    </div>` : ""}

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
  selectedRuleName = name;
  renderDetail(rule);
  renderList(); // Re-render to update active state
  location.hash = name;
  closeMobileMenu(); // Close menu on mobile after selecting
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
      const sortedRules = [...rules].sort((a, b) =>
        a.name.localeCompare(b.name),
      );
      selectRule(sortedRules[0].name);
    }
  });

searchInput.addEventListener("input", applyFilters);
categoryFilter.addEventListener("change", applyFilters);
