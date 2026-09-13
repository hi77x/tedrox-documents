# Localization

English is the canonical language. Russian ships fully polished before any
release; other languages are added only when a translator can maintain them.

## Layout

```text
packages/i18n/
  en.json    # canonical keys
  ru.json    # complete translation
```

The static landing imports them through `scripts/sync-i18n.mjs`, which copies
the dictionaries into `apps/web/i18n/` and **fails the build** when:

- an English key is missing from Russian,
- Russian contains an orphan key that English does not have.

CI runs the same script, so dictionaries cannot drift.

## Key namespaces

| Prefix | Use |
| --- | --- |
| `meta.*` | Page metadata (title, description) |
| `nav.*` | Navigation |
| `hero.*` | Landing hero |
| `feature.*` | Feature cards |
| `formats.*` | Format sections |
| `privacy.*` | Privacy section |
| `cli.*` | CLI section |
| `download.*` | Download cards |
| `faq.*` | FAQ |
| `errors.*` | Future runtime error strings |
| `common.*` | Shared buttons and labels |

Names are stable identifiers, not sentences; translations never change the key.

## Adding a language

1. Copy `packages/i18n/en.json` to `xx.json` and translate values only.
2. Add `xx` to the `languages` list in `scripts/sync-i18n.mjs` and to
   `SUPPORTED` in `apps/web/app.js`.
3. Run `node scripts/sync-i18n.mjs` — it validates completeness.
4. Translate only what you can maintain; half-machine-translated dictionaries
   are rejected in review.

## Placeholders

Placeholders use `{name}` syntax and must survive translation unchanged. When
runtime strings are introduced, the validator will compare placeholder sets
between languages in the same way it compares keys today.

## Text guidelines

- Keep sentences short and concrete.
- Error messages state cause and next step, for example: "This PDF is encrypted.
  Enter its password before extracting pages."
- Do not translate product names (`TEDROX Documents`, `tdx-doc`).
- Use the imperative for actions: `Merge PDFs`, `Объединить PDF`.
