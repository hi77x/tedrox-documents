/* TEDROX Documents landing — i18n, release links, OS hints. No dependencies. */
(() => {
  "use strict";

  const DEFAULT_LANG = "en";
  const SUPPORTED = ["en", "ru"];
  const dictionaries = {};
  const fallback = {};

  function detectLanguage() {
    const stored = localStorage.getItem("tdx-lang");
    if (stored && SUPPORTED.includes(stored)) return stored;
    for (const candidate of navigator.languages || [navigator.language || DEFAULT_LANG]) {
      const code = String(candidate).slice(0, 2).toLowerCase();
      if (SUPPORTED.includes(code)) return code;
    }
    return DEFAULT_LANG;
  }

  async function loadDictionary(lang) {
    if (dictionaries[lang]) return dictionaries[lang];
    const response = await fetch(`i18n/${lang}.json`, { cache: "no-cache" });
    if (!response.ok) throw new Error(`i18n ${lang}: ${response.status}`);
    dictionaries[lang] = await response.json();
    return dictionaries[lang];
  }

  function translate(lang) {
    const dict = dictionaries[lang] || fallback;
    document.documentElement.lang = lang;
    document.title = dict["meta.title"] || fallback["meta.title"] || document.title;

    document.querySelectorAll("[data-i18n]").forEach((element) => {
      const key = element.getAttribute("data-i18n");
      const value = dict[key] || fallback[key];
      if (value) element.textContent = value;
    });

    document.querySelectorAll("[data-i18n-attr]").forEach((element) => {
      const spec = element.getAttribute("data-i18n-attr");
      for (const pair of spec.split(",")) {
        const [attribute, key] = pair.split(":").map((part) => part.trim());
        const value = dict[key] || fallback[key];
        if (attribute && value) element.setAttribute(attribute, value);
      }
    });

    document.querySelectorAll(".lang-switch button").forEach((button) => {
      const isActive = button.dataset.lang === lang;
      button.classList.toggle("active", isActive);
      button.setAttribute("aria-pressed", String(isActive));
    });
  }

  function setLanguage(lang) {
    try {
      localStorage.setItem("tdx-lang", lang);
    } catch {
      /* storage unavailable; ignore */
    }
    translate(lang);
  }

  function recommendedPlatform() {
    const platform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
    const userAgent = navigator.userAgent.toLowerCase();
    if (platform.includes("win") || userAgent.includes("windows")) return "windows";
    if (platform.includes("linux") || userAgent.includes("linux")) return "linux";
    if (/android|iphone|ipad/.test(userAgent)) return "android";
    return null;
  }

  function markRecommended(target) {
    if (!target) return;
    const card = document.querySelector(`[data-download="${target}"]`)?.closest(".download-card");
    if (!card) return;
    card.style.borderColor = "var(--accent)";
    const badge = document.createElement("em");
    badge.style.cssText = "font-size:.78rem;color:var(--accent-strong);font-style:normal;font-weight:600";
    badge.setAttribute("data-i18n", "download.recommended");
    badge.textContent = dictionaries[document.documentElement.lang]?.["download.recommended"] || "Recommended for your system";
    card.querySelector("h3")?.after(badge);
  }

  async function applyReleases() {
    let manifest = null;
    try {
      const response = await fetch("releases.json", { cache: "no-cache" });
      if (response.ok) manifest = await response.json();
    } catch {
      manifest = null;
    }
    const versionLabel = document.getElementById("release-version");
    if (manifest?.version) {
      if (versionLabel) versionLabel.textContent = manifest.version;
      const windowsLink = document.querySelector('[data-download="windows"]');
      const asset = (manifest.assets || []).find((item) => /windows.*\.zip$/i.test(item.name));
      if (windowsLink && asset?.url) windowsLink.href = asset.url;
    } else if (versionLabel) {
      versionLabel.textContent = "0.1.x";
    }
  }

  async function main() {
    try {
      fallback.en = await loadDictionary("en");
    } catch {
      fallback.en = {};
    }

    const initial = detectLanguage();
    if (initial !== "en") {
      try {
        await loadDictionary(initial);
      } catch {
        /* keep English */
      }
    }
    translate(initial);

    document.querySelectorAll(".lang-switch button").forEach((button) => {
      button.addEventListener("click", async () => {
        const lang = button.dataset.lang;
        try {
          await loadDictionary(lang);
        } catch {
          return;
        }
        setLanguage(lang);
      });
    });

    markRecommended(recommendedPlatform());
    await applyReleases();
  }

  main();
})();
