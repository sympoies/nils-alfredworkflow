import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const scraperPath = path.resolve(__dirname, '..', 'cambridge_scraper.mjs');
const fixturePath = (name) => path.join(__dirname, 'fixtures', name);

function runScraper(args) {
  const result = spawnSync(process.execPath, [scraperPath, ...args], {
    encoding: 'utf8',
  });

  let payload = null;
  const stdout = result.stdout.trim();
  if (stdout.startsWith('{') || stdout.startsWith('[')) {
    payload = JSON.parse(stdout);
  }

  return {
    ...result,
    payload,
  };
}

test('suggest emits stable success JSON with fixture html', () => {
  const run = runScraper([
    'suggest',
    '--query',
    'open',
    '--mode',
    'english',
    '--max-results',
    '2',
    '--fixture-html',
    fixturePath('suggest-english-open.html'),
  ]);

  assert.equal(run.status, 0);
  assert.equal(run.payload.ok, true);
  assert.equal(run.payload.stage, 'suggest');
  assert.equal(run.payload.items.length, 2);
  assert.equal(run.payload.items[0].entry, 'open');
});

test('suggest keeps exact dictionary entries ahead of browse links on direct pages', () => {
  const run = runScraper([
    'suggest',
    '--query',
    'symphony',
    '--mode',
    'english-chinese-traditional',
    '--fixture-html',
    fixturePath('suggest-english-chinese-traditional-direct-symphony.html'),
  ]);

  assert.equal(run.status, 0);
  assert.equal(run.payload.ok, true);
  assert.deepEqual(
    run.payload.items.map((item) => item.entry),
    ['symphony', 'sympathize', 'sympathizer', 'sympathy'],
  );
});

test('suggest maps spellcheck pages to q-based suggestions instead of unrelated dictionary links', () => {
  const run = runScraper([
    'suggest',
    '--query',
    'symph',
    '--mode',
    'english-chinese-traditional',
    '--fixture-html',
    fixturePath('suggest-english-chinese-traditional-spellcheck-symph.html'),
  ]);

  assert.equal(run.status, 0);
  assert.equal(run.payload.ok, true);
  assert.deepEqual(
    run.payload.items.map((item) => item.entry),
    ['lymph', 'sympathy', 'symphony'],
  );
});

test('define emits stable success JSON with fixture html', () => {
  const run = runScraper([
    'define',
    '--entry',
    'open',
    '--mode',
    'english-chinese-traditional',
    '--fixture-html',
    fixturePath('define-english-chinese-traditional-open.html'),
  ]);

  assert.equal(run.status, 0);
  assert.equal(run.payload.ok, true);
  assert.equal(run.payload.stage, 'define');
  assert.equal(run.payload.entry.headword, 'open');
  assert.ok(run.payload.entry.definitions.length >= 2);
  assert.ok(run.payload.entry.examples.length >= 2);
});

test('invalid mode emits structured error JSON', () => {
  const run = runScraper([
    'suggest',
    '--query',
    'open',
    '--mode',
    'invalid-mode',
    '--fixture-html',
    fixturePath('suggest-english-open.html'),
  ]);

  assert.equal(run.status, 2);
  assert.equal(run.payload.ok, false);
  assert.equal(run.payload.error.code, 'invalid_args');
});
