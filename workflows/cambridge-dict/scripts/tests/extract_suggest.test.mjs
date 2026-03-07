import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { extractSuggestFromHtml } from '../lib/extract_suggest.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const fixture = async (name) => {
  const fullPath = path.join(__dirname, 'fixtures', name);
  return readFile(fullPath, 'utf8');
};

test('extractSuggestFromHtml returns deduped ordered candidates (english)', async () => {
  const html = await fixture('suggest-english-open.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english',
    maxResults: 10,
  });

  assert.equal(items.length, 3);
  assert.deepEqual(
    items.map((item) => item.entry),
    ['open', 'open up', 'open-minded'],
  );
  assert.ok(items[0].url.includes('/dictionary/english/open'));
});

test('extractSuggestFromHtml honors maxResults clamp', async () => {
  const html = await fixture('suggest-english-open.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english',
    maxResults: 2,
  });

  assert.equal(items.length, 2);
});

test('extractSuggestFromHtml supports english-chinese-traditional mode', async () => {
  const html = await fixture('suggest-english-chinese-traditional-open.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english-chinese-traditional',
    maxResults: 10,
  });

  assert.equal(items.length, 3);
  assert.ok(items[0].url.includes('/dictionary/english-chinese-traditional/open'));
});

test('extractSuggestFromHtml prefers direct-entry headword over dictionary heading copy', async () => {
  const html = await fixture('suggest-english-direct-symphony.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english',
    maxResults: 10,
    pageUrl: 'https://dictionary.cambridge.org/dictionary/english/symphony',
  });

  assert.deepEqual(
    items.map((item) => item.entry),
    ['symphony', 'symphonic', 'symphony orchestra'],
  );
});

test('extractSuggestFromHtml keeps direct bilingual pages free of translation heading rows', async () => {
  const html = await fixture('suggest-english-chinese-traditional-direct-symphony.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english-chinese-traditional',
    maxResults: 10,
    pageUrl: 'https://dictionary.cambridge.org/dictionary/english-chinese-traditional/symphony',
  });

  assert.deepEqual(
    items.map((item) => item.entry),
    ['symphony', 'sympathize', 'sympathizer', 'sympathy'],
  );
});

test('extractSuggestFromHtml reads spellcheck suggestions and ignores unrelated dictionary links', async () => {
  const html = await fixture('suggest-english-chinese-traditional-spellcheck-symph.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english-chinese-traditional',
    maxResults: 10,
    pageUrl: 'https://dictionary.cambridge.org/spellcheck/english-chinese-traditional/?q=symph',
  });

  assert.deepEqual(
    items.map((item) => item.entry),
    ['lymph', 'sympathy', 'symphony'],
  );
});

test('extractSuggestFromHtml uses canonical entry on direct english pages', async () => {
  const html = await fixture('suggest-english-direct-symphony.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english',
    maxResults: 10,
  });

  assert.deepEqual(
    items.map((item) => item.entry),
    ['symphony', 'symphonic', 'symphony orchestra'],
  );
  assert.equal(items[0].entry, 'symphony');
  assert.ok(!items.some((item) => item.entry.startsWith('meaning of ')));
});

test('extractSuggestFromHtml ignores translation heading noise on direct bilingual pages', async () => {
  const html = await fixture('suggest-english-chinese-traditional-direct-symphony.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english-chinese-traditional',
    maxResults: 10,
  });

  assert.deepEqual(
    items.map((item) => item.entry),
    ['symphony', 'sympathize', 'sympathizer', 'sympathy'],
  );
  assert.ok(!items.some((item) => item.entry.startsWith('translation of ')));
});

test('extractSuggestFromHtml prefers spellcheck suggestion links over unrelated dictionary links', async () => {
  const html = await fixture('suggest-english-chinese-traditional-spellcheck-symph.html');
  const items = extractSuggestFromHtml({
    html,
    mode: 'english-chinese-traditional',
    maxResults: 10,
  });

  assert.deepEqual(
    items.map((item) => item.entry),
    ['lymph', 'sympathy', 'symphony'],
  );
  assert.ok(!items.some((item) => item.entry === 'at sixes and sevens'));
});
