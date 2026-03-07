import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { extractDefineFromHtml } from '../lib/extract_define.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const fixture = async (name) => {
  const fullPath = path.join(__dirname, 'fixtures', name);
  return readFile(fullPath, 'utf8');
};

test('extractDefineFromHtml parses english entry fields', async () => {
  const html = await fixture('define-english-open.html');
  const entry = extractDefineFromHtml({
    html,
    mode: 'english',
    entry: 'open',
  });

  assert.equal(entry.headword, 'open');
  assert.equal(entry.partOfSpeech, 'adjective');
  assert.deepEqual(entry.phonetics, ['/əʊ.pən/']);
  assert.deepEqual(entry.definitions, ['not closed or fastened', 'ready to allow people in']);
  assert.deepEqual(entry.examples, ['an open door/window', 'The museum is open until six.']);
  assert.equal(entry.url, 'https://dictionary.cambridge.org/dictionary/english/open');
});

test('extractDefineFromHtml parses english-chinese-traditional entry fields', async () => {
  const html = await fixture('define-english-chinese-traditional-open.html');
  const entry = extractDefineFromHtml({
    html,
    mode: 'english-chinese-traditional',
    entry: 'open',
  });

  assert.equal(entry.headword, 'open');
  assert.equal(entry.partOfSpeech, 'adjective');
  assert.ok(entry.definitions.includes('not closed | 開著的'));
  assert.ok(entry.definitions.includes('ready for business | 營業中的'));
  assert.ok(entry.examples.includes('an open door/window | 開著的門／窗'));
  assert.ok(entry.examples.includes('The museum is open until six. | 博物館營業到六點。'));
  assert.equal(
    entry.url,
    'https://dictionary.cambridge.org/dictionary/english-chinese-traditional/open',
  );
});

test('extractDefineFromHtml suppresses translation-only overflow in traditional mode', () => {
  const html = `
    <!doctype html>
    <html>
      <head>
        <title>open | Cambridge Dictionary</title>
      </head>
      <body>
        <div class="entry-body">
          <h1 class="di-title"><span class="hw">open</span></h1>
          <div class="sense-body">
            <div class="def">available for use</div>
            <div class="trans">可供使用</div>
            <div class="trans">額外翻譯列</div>
          </div>
        </div>
      </body>
    </html>
  `;

  const entry = extractDefineFromHtml({
    html,
    mode: 'english-chinese-traditional',
    entry: 'open',
  });

  assert.deepEqual(entry.definitions, ['available for use | 可供使用']);
});

test('extractDefineFromHtml keeps full definition text and skips def-info tokens', () => {
  const html = `
    <!doctype html>
    <html>
      <head>
        <title>take | Cambridge Dictionary</title>
        <link rel="canonical" href="https://dictionary.cambridge.org/dictionary/english/take" />
      </head>
      <body>
        <div class="entry-body">
          <h1 class="di-title"><span class="hw">take</span></h1>
          <div class="posgram"><span class="pos">verb</span></div>
          <div class="def-head"><span class="def-info">Add to word list</span></div>
          <div class="def-block">
            <div class="def ddef_d db">
              to remove <span class="gram">something</span>, especially without permission
            </div>
          </div>
        </div>
      </body>
    </html>
  `;

  const entry = extractDefineFromHtml({
    html,
    mode: 'english',
    entry: 'take',
  });

  assert.deepEqual(entry.definitions, ['to remove something, especially without permission']);
  assert.deepEqual(entry.examples, []);
  assert.ok(!entry.definitions.includes('Add to word list'));
});

test('extractDefineFromHtml supports list-item examples without nested eg spans', () => {
  const html = `
    <!doctype html>
    <html>
      <head>
        <title>take off | Cambridge Dictionary</title>
      </head>
      <body>
        <div class="entry-body">
          <h1 class="di-title"><span class="hw">take off</span></h1>
          <div class="sense-body">
            <div class="def">to remove something, especially clothes</div>
            <li class="eg dexamp hax">He took off his shoes to cool his sweaty feet.</li>
          </div>
        </div>
      </body>
    </html>
  `;

  const entry = extractDefineFromHtml({
    html,
    mode: 'english',
    entry: 'take off',
  });

  assert.deepEqual(entry.examples, ['He took off his shoes to cool his sweaty feet.']);
});
