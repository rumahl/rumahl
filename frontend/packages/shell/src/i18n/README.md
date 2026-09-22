# Shell localization

English is the default and fallback language. German is the second supported language.

To add a language:

1. Add a catalog in `locales/` that `satisfies Messages` from `locales/en.ts`.
2. Register its language code in `catalogs` in `index.tsx`.
3. Add rendering coverage for the language in `App.test.tsx`.

The English catalog is the compile-time message schema. A catalog with a missing key fails the
TypeScript build. Runtime values such as app counts and timestamps belong to the authenticated
shell snapshot, not to a catalog. Plural selection, localized number formatting, and relative time
formatting are handled centrally by the i18n provider.
