import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  ERROR_CODES,
  classifyHtmlBarrier,
  classifyRuntimeError,
  classifyScraperError,
} from '../lib/error_classify.mjs';

test('classifyHtmlBarrier detects anti-bot page', () => {
  const classified = classifyHtmlBarrier('<html><title>Attention Required! | Cloudflare</title></html>');
  assert.equal(classified.code, ERROR_CODES.ANTI_BOT);
  assert.equal(classified.retriable, true);
});

test('classifyHtmlBarrier ignores benign cloudflare references', () => {
  const classified = classifyHtmlBarrier('<html><script src=\"https://static.cloudflareinsights.com/beacon.min.js\"></script></html>');
  assert.equal(classified, null);
});

test('classifyHtmlBarrier ignores cookie text when dictionary content exists', () => {
  const html = '<html><link rel=\"canonical\" href=\"https://dictionary.cambridge.org/dictionary/english/open\" /><div class=\"entry-body\"><div class=\"headword\">open</div><div>cookie consent</div></div></html>';
  const classified = classifyHtmlBarrier(html);
  assert.equal(classified, null);
});

test('classifyHtmlBarrier ignores spellcheck suggestion pages even when cookie text appears elsewhere', () => {
  const fixtureHtml = readFileSync(
    new URL('./fixtures/suggest-english-chinese-traditional-spellcheck-symph.html', import.meta.url),
    'utf8',
  );
  const classified = classifyHtmlBarrier(`${fixtureHtml}<div>cookie consent preferences</div>`);
  assert.equal(classified, null);
});

test('classifyHtmlBarrier detects cookie consent wall', () => {
  const classified = classifyHtmlBarrier('<html>Please enable cookies and consent preferences</html>');
  assert.equal(classified.code, ERROR_CODES.COOKIE_WALL);
});

test('classifyRuntimeError detects timeout and network', () => {
  assert.equal(classifyRuntimeError(new Error('navigation timeout exceeded')).code, ERROR_CODES.TIMEOUT);
  assert.equal(classifyRuntimeError(new Error('net::ERR_NAME_NOT_RESOLVED')).code, ERROR_CODES.NETWORK);
});

test('classifyScraperError prioritizes html barrier over runtime error', () => {
  const classified = classifyScraperError({
    error: new Error('navigation timeout exceeded'),
    html: '<html><title>Attention Required! | Cloudflare</title></html>',
  });
  assert.equal(classified.code, ERROR_CODES.ANTI_BOT);
});
