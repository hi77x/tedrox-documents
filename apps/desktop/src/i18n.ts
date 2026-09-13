import en from "./i18n/en.json";
import ru from "./i18n/ru.json";

export type Language = "en" | "ru";

const dictionaries: Record<Language, Record<string, string>> = { en, ru };

let current: Language = "en";

export function detectLanguage(): Language {
  const stored = localStorage.getItem("tdx-lang");
  if (stored === "en" || stored === "ru") return stored;
  const preferred = navigator.language.slice(0, 2).toLowerCase();
  return preferred === "ru" ? "ru" : "en";
}

export function setLanguage(language: Language) {
  current = language;
  localStorage.setItem("tdx-lang", language);
  document.documentElement.lang = language;
}

export function t(key: string): string {
  return dictionaries[current][key] ?? dictionaries.en[key] ?? key;
}
