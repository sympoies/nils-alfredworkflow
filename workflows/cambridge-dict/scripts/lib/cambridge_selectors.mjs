import { normalizeMode } from './cambridge_routes.mjs';

const BASE_SELECTORS = Object.freeze({
  suggest: Object.freeze({
    searchResultContainers: Object.freeze([
      '.search_results',
    ]),
    searchResultLinks: Object.freeze([
      'a.entry-link[href*="/dictionary/"]',
    ]),
    directEntryContainers: Object.freeze([
      '.entry-body',
      '.entry',
    ]),
    directHeadwords: Object.freeze([
      '.entry-body .headword',
      '.entry-body .hw',
      '.entry .headword',
    ]),
    directBrowseContainers: Object.freeze([
      '.dbrowse',
    ]),
    directBrowseLinks: Object.freeze([
      'a[href*="/dictionary/"]',
    ]),
    spellcheckSuggestionLists: Object.freeze([
      '.hul-u',
    ]),
    spellcheckSuggestionLinks: Object.freeze([
      'a[href*="/search/"]',
    ]),
  }),
  define: Object.freeze({
    headword: Object.freeze([
      '.entry-body .headword',
      '.head .hw',
      '.di-title .hw',
      'h1 .hw',
    ]),
    partOfSpeech: Object.freeze([
      '.entry-body .pos',
      '.posgram .pos',
      '.pos-header .pos',
    ]),
    phonetics: Object.freeze([
      '.entry-body .ipa',
      '.pron .ipa',
      '.dpron-i .ipa',
    ]),
    definitions: Object.freeze([
      '.entry-body .def',
      '.sense-body .def',
      '.def-block .def',
      '.def-body .def',
      '.def.ddef_d',
    ]),
    exampleContainers: Object.freeze([
      '.dexamp',
    ]),
    exampleText: Object.freeze([
      '.eg',
      '.deg',
    ]),
    exampleTranslations: Object.freeze([
      '.trans',
      '.dtrans',
    ]),
    canonicalUrl: Object.freeze([
      'link[rel="canonical"]',
      'meta[property="og:url"]',
      'a[href*="/dictionary/"]',
    ]),
  }),
});

const MODE_OVERRIDES = Object.freeze({
  english: Object.freeze({}),
  'english-chinese-traditional': Object.freeze({
    define: Object.freeze({
      definitions: Object.freeze([
        '.entry-body .trans',
        '.entry-body .def',
        '.sense-body .trans',
        '.def-body .trans',
      ]),
    }),
  }),
});

function mergeUnique(baseList, overrideList) {
  const merged = [...(overrideList || []), ...(baseList || [])];
  return [...new Set(merged)];
}

function mergeStage(baseStage, overrideStage) {
  const result = {};
  const keys = new Set([...Object.keys(baseStage || {}), ...Object.keys(overrideStage || {})]);
  for (const key of keys) {
    result[key] = mergeUnique(baseStage?.[key], overrideStage?.[key]);
  }
  return result;
}

export function selectorsForMode(mode) {
  const normalizedMode = normalizeMode(mode);
  const override = MODE_OVERRIDES[normalizedMode] || {};
  return {
    suggest: mergeStage(BASE_SELECTORS.suggest, override.suggest),
    define: mergeStage(BASE_SELECTORS.define, override.define),
  };
}

export function selectorsForStage({ mode, stage }) {
  const selectorSet = selectorsForMode(mode);
  if (!selectorSet[stage]) {
    throw new Error(`unknown stage: ${stage}`);
  }
  return selectorSet[stage];
}

export { BASE_SELECTORS, MODE_OVERRIDES };
