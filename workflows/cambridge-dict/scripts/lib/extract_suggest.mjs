import { CAMBRIDGE_BASE_URL, buildDefineUrl, normalizeMode, sanitizeEntry } from './cambridge_routes.mjs';
import { selectorsForStage } from './cambridge_selectors.mjs';

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function decodeHtmlEntities(value) {
  return value
    .replace(/&nbsp;/gi, ' ')
    .replace(/&amp;/gi, '&')
    .replace(/&lt;/gi, '<')
    .replace(/&gt;/gi, '>')
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/gi, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_, hex) =>
      String.fromCodePoint(Number.parseInt(hex, 16)),
    )
    .replace(/&#([0-9]+);/g, (_, code) => String.fromCodePoint(Number.parseInt(code, 10)));
}

function stripTags(value) {
  return decodeHtmlEntities(String(value ?? '').replace(/<[^>]+>/g, ' '));
}

function normalizeText(value) {
  return stripTags(value)
    .replace(/\s+/g, ' ')
    .trim();
}

function primaryClassFromSelector(selector) {
  const classes = [...String(selector).matchAll(/\.([a-zA-Z][\w-]*)/g)].map((match) => match[1]);
  if (classes.length === 0) {
    return '';
  }
  return classes[classes.length - 1];
}

const VOID_TAGS = new Set([
  'area',
  'base',
  'br',
  'col',
  'embed',
  'hr',
  'img',
  'input',
  'link',
  'meta',
  'param',
  'source',
  'track',
  'wbr',
]);

function hasExactClassToken(rawClassList, className) {
  return String(rawClassList)
    .split(/\s+/)
    .some((token) => token === className);
}

function isSelfClosingOpenTag(openTag, tagName) {
  return /\/\s*>$/.test(openTag) || VOID_TAGS.has(String(tagName).toLowerCase());
}

function findMatchingCloseIndex(html, tagName, contentStartIndex) {
  const normalizedTagName = String(tagName).toLowerCase();
  if (VOID_TAGS.has(normalizedTagName)) {
    return contentStartIndex;
  }

  const pattern = new RegExp(`<\\/?${escapeRegExp(normalizedTagName)}\\b[^>]*>`, 'gi');
  pattern.lastIndex = contentStartIndex;

  let depth = 1;
  let match;
  while ((match = pattern.exec(html)) !== null) {
    const token = match[0];
    if (token.startsWith('</')) {
      depth -= 1;
      if (depth === 0) {
        return match.index;
      }
      continue;
    }

    if (!/\/\s*>$/.test(token)) {
      depth += 1;
    }
  }

  return -1;
}

function collectBlocksByClass(html, className, limit = Number.POSITIVE_INFINITY) {
  const pattern = /<([a-zA-Z][\w:-]*)\b[^>]*\bclass=(['"])([^'"]+)\2[^>]*>/gi;
  const blocks = [];

  for (const match of html.matchAll(pattern)) {
    const openTag = match[0];
    const tagName = String(match[1]).toLowerCase();
    const rawClassList = match[3];
    if (!hasExactClassToken(rawClassList, className)) {
      continue;
    }
    if (isSelfClosingOpenTag(openTag, tagName)) {
      continue;
    }

    const contentStartIndex = (match.index ?? 0) + openTag.length;
    const contentEndIndex = findMatchingCloseIndex(html, tagName, contentStartIndex);
    if (contentEndIndex < contentStartIndex) {
      continue;
    }

    blocks.push(html.slice(contentStartIndex, contentEndIndex));
    if (blocks.length >= limit) {
      break;
    }
  }

  return blocks;
}

function collectClassTexts(html, className, limit = Number.POSITIVE_INFINITY) {
  const values = [];
  for (const block of collectBlocksByClass(html, className, limit)) {
    const normalized = normalizeText(block);
    if (normalized) {
      values.push(normalized);
    }
  }
  return values;
}

function collectBlocksForSelectors(html, selectors, limit = Number.POSITIVE_INFINITY) {
  const output = [];

  for (const selector of selectors) {
    const className = primaryClassFromSelector(selector);
    if (!className) {
      continue;
    }

    for (const block of collectBlocksByClass(html, className, limit)) {
      output.push(block);
      if (output.length >= limit) {
        return output;
      }
    }
  }

  return output;
}

function extractClassList(openTag) {
  const match = String(openTag).match(/\bclass=(['"])([^'"]+)\1/i);
  return match?.[2] ?? '';
}

function collectAnchors(html, { className = '', hrefPattern = null } = {}) {
  const pattern = /<a\b[^>]*href=(['"])([^'"]+)\1[^>]*>([\s\S]*?)<\/a>/gi;
  const anchors = [];

  for (const match of html.matchAll(pattern)) {
    const openTag = match[0].match(/^<a\b[^>]*>/i)?.[0] ?? '';
    const href = match[2];
    if (hrefPattern && !hrefPattern.test(href)) {
      continue;
    }
    if (className && !hasExactClassToken(extractClassList(openTag), className)) {
      continue;
    }

    const text = normalizeText(match[3]);
    if (!text) {
      continue;
    }

    anchors.push({
      href,
      text,
      html: match[3],
    });
  }

  return anchors;
}

function extractEntryFromDictionaryUrl(href, mode) {
  const normalizedMode = normalizeMode(mode);
  const pattern = new RegExp(`/dictionary/${escapeRegExp(normalizedMode)}/([^/?#]+)`, 'i');
  const match = String(href).match(pattern);
  if (!match || !match[1]) {
    return null;
  }

  const decoded = decodeURIComponent(match[1]).replace(/-/g, ' ').trim();
  return decoded || null;
}

function extractEntryFromSearchUrl(href) {
  try {
    const url = new URL(href, CAMBRIDGE_BASE_URL);
    const entry = url.searchParams.get('q');
    return entry ? sanitizeEntry(entry) : null;
  } catch {
    return null;
  }
}

function absolutizeUrl(href) {
  const raw = String(href ?? '').trim();
  if (!raw) {
    return '';
  }
  if (/^https?:\/\//i.test(raw)) {
    return raw;
  }
  if (raw.startsWith('/')) {
    return `${CAMBRIDGE_BASE_URL}${raw}`;
  }
  return `${CAMBRIDGE_BASE_URL}/${raw}`;
}

function findCanonicalUrl(html) {
  const linkMatch = html.match(/<link\b[^>]*rel=(['"])canonical\1[^>]*href=(['"])([^'"]+)\2[^>]*>/i);
  if (linkMatch && linkMatch[3]) {
    return absolutizeUrl(linkMatch[3]);
  }

  const metaMatch = html.match(/<meta\b[^>]*property=(['"])og:url\1[^>]*content=(['"])([^'"]+)\2[^>]*>/i);
  if (metaMatch && metaMatch[3]) {
    return absolutizeUrl(metaMatch[3]);
  }

  return '';
}

function clampMaxResults(maxResults) {
  const parsed = Number.parseInt(String(maxResults ?? '8'), 10);
  if (!Number.isFinite(parsed)) {
    return 8;
  }
  return Math.min(20, Math.max(1, parsed));
}

function dedupeKey(entry) {
  return sanitizeEntry(entry).replace(/-/g, ' ');
}

function normalizeHeadwordCandidate(value) {
  const normalized = normalizeText(value);
  if (!normalized) {
    return '';
  }

  const lower = normalized.toLowerCase();
  if (
    lower.startsWith('meaning of ') ||
    lower.startsWith('translation of ') ||
    lower.includes(' dictionary')
  ) {
    return '';
  }

  return normalized;
}

function extractSearchResultCandidates(source, mode) {
  const selectors = selectorsForStage({ mode, stage: 'suggest' });
  const blocks = collectBlocksForSelectors(source, selectors.searchResultContainers);
  const candidates = [];
  const linkClasses = selectors.searchResultLinks
    .map(primaryClassFromSelector)
    .filter(Boolean);

  for (const block of blocks) {
    const anchors = [];
    for (const className of linkClasses) {
      anchors.push(
        ...collectAnchors(block, {
          className,
          hrefPattern: /\/dictionary\//i,
        }),
      );
    }

    for (const anchor of anchors) {
      const headword = collectClassTexts(anchor.html, 'hw', 1)
        .map((value) => normalizeHeadwordCandidate(value))
        .find(Boolean);
      const entry = headword || extractEntryFromDictionaryUrl(anchor.href, mode);
      if (!entry) {
        continue;
      }

      candidates.push({
        entry,
        label: headword || anchor.text || entry,
        url: absolutizeUrl(anchor.href),
      });
    }

    if (anchors.length > 0) {
      continue;
    }

    for (const text of collectClassTexts(block, 'hw')) {
      const entry = normalizeHeadwordCandidate(text);
      if (!entry) {
        continue;
      }
      candidates.push({ entry, label: entry, url: '' });
    }
  }

  return candidates;
}

function looksLikeSpellcheckPage(source) {
  return (
    /Search suggestions for/i.test(source) ||
    /Did you spell it correctly/i.test(source) ||
    /Alternative spellings/i.test(source)
  );
}

function extractSpellcheckCandidates(source, mode) {
  if (!looksLikeSpellcheckPage(source)) {
    return [];
  }

  const selectors = selectorsForStage({ mode, stage: 'suggest' });
  const headingIndex = source.toLowerCase().indexOf('search suggestions for');
  const scopedSource = headingIndex >= 0 ? source.slice(headingIndex) : source;
  const listBlock = collectBlocksForSelectors(scopedSource, selectors.spellcheckSuggestionLists, 1)[0];
  if (!listBlock) {
    return [];
  }

  const candidates = [];
  for (const selector of selectors.spellcheckSuggestionLinks) {
    const className = primaryClassFromSelector(selector);
    const anchors = collectAnchors(listBlock, {
      className,
      hrefPattern: /\/search\/[^"'?#]+\/direct\/\?q=/i,
    });

    for (const anchor of anchors) {
      const entry = extractEntryFromSearchUrl(anchor.href);
      if (!entry) {
        continue;
      }

      candidates.push({
        entry,
        label: anchor.text || entry,
        url: buildDefineUrl({ entry, mode }),
      });
    }
  }

  return candidates;
}

function extractExactEntryCandidate(source, mode, pageUrl) {
  const urlCandidates = [findCanonicalUrl(source), absolutizeUrl(pageUrl)].filter(Boolean);
  const selectors = selectorsForStage({ mode, stage: 'suggest' });
  const directEntryBlock = collectBlocksForSelectors(source, selectors.directEntryContainers, 1)[0] || source;

  for (const rawUrl of urlCandidates) {
    const entry = extractEntryFromDictionaryUrl(rawUrl, mode);
    if (!entry) {
      continue;
    }

    let label = entry;
    for (const selector of selectors.directHeadwords) {
      const className = primaryClassFromSelector(selector);
      if (!className) {
        continue;
      }

      for (const text of collectClassTexts(directEntryBlock, className)) {
        const normalized = normalizeHeadwordCandidate(text);
        if (!normalized) {
          continue;
        }
        label = normalized;
        break;
      }

      if (label !== entry) {
        break;
      }
    }

    return {
      entry,
      label,
      url: rawUrl,
    };
  }

  return null;
}

function extractBrowseCandidates(source, mode) {
  const selectors = selectorsForStage({ mode, stage: 'suggest' });
  const candidates = [];
  const browseBlocks = collectBlocksForSelectors(source, selectors.directBrowseContainers);
  const linkClasses = selectors.directBrowseLinks
    .map(primaryClassFromSelector)
    .filter(Boolean);

  for (const block of browseBlocks) {
    const anchors = [];
    for (const className of linkClasses) {
      anchors.push(
        ...collectAnchors(block, {
          className,
          hrefPattern: /\/dictionary\//i,
        }),
      );
    }
    if (anchors.length === 0) {
      anchors.push(...collectAnchors(block, { hrefPattern: /\/dictionary\//i }));
    }

    for (const anchor of anchors) {
      const entry = extractEntryFromDictionaryUrl(anchor.href, mode);
      if (!entry) {
        continue;
      }
      candidates.push({
        entry,
        label: anchor.text || entry,
        url: absolutizeUrl(anchor.href),
      });
    }
  }

  return candidates;
}

function fallbackHeadwordCandidates(source, mode) {
  const selectors = selectorsForStage({ mode, stage: 'suggest' });
  const directEntryBlock = collectBlocksForSelectors(source, selectors.directEntryContainers, 1)[0] || source;
  const candidates = [];

  for (const selector of selectors.directHeadwords) {
    const className = primaryClassFromSelector(selector);
    if (!className) {
      continue;
    }

    for (const text of collectClassTexts(directEntryBlock, className, 2)) {
      const entry = normalizeHeadwordCandidate(text);
      if (!entry) {
        continue;
      }
      candidates.push({ entry, label: entry, url: '' });
    }
  }

  return candidates;
}

function finalizeCandidates(candidates, mode, limit) {
  const deduped = [];
  const seen = new Set();

  for (const candidate of candidates) {
    let entry;
    try {
      entry = sanitizeEntry(candidate.entry);
    } catch {
      continue;
    }

    const key = dedupeKey(entry);
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    deduped.push({
      entry,
      label: candidate.label || entry,
      url: candidate.url || buildDefineUrl({ entry, mode }),
    });

    if (deduped.length >= limit) {
      break;
    }
  }

  return deduped;
}

export function extractSuggestFromHtml({ html, mode, maxResults, pageUrl = '' }) {
  const normalizedMode = normalizeMode(mode);
  const source = String(html ?? '');
  const limit = clampMaxResults(maxResults);

  const searchResultCandidates = extractSearchResultCandidates(source, normalizedMode);
  if (searchResultCandidates.length > 0) {
    return finalizeCandidates(searchResultCandidates, normalizedMode, limit);
  }

  const spellcheckCandidates = extractSpellcheckCandidates(source, normalizedMode);
  if (spellcheckCandidates.length > 0) {
    return finalizeCandidates(spellcheckCandidates, normalizedMode, limit);
  }
  if (looksLikeSpellcheckPage(source)) {
    return [];
  }

  const directCandidates = [];
  const exactEntry = extractExactEntryCandidate(source, normalizedMode, pageUrl);
  if (exactEntry) {
    directCandidates.push(exactEntry);
  }
  directCandidates.push(...extractBrowseCandidates(source, normalizedMode));
  if (directCandidates.length > 0) {
    return finalizeCandidates(directCandidates, normalizedMode, limit);
  }

  return finalizeCandidates(fallbackHeadwordCandidates(source, normalizedMode), normalizedMode, limit);
}
