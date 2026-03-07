#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { buildDefineUrl, buildSuggestUrl, normalizeMode, sanitizeEntry } from './lib/cambridge_routes.mjs';
import { asStructuredError, classifyHtmlBarrier } from './lib/error_classify.mjs';
import { extractDefineFromHtml } from './lib/extract_define.mjs';
import { extractSuggestFromHtml } from './lib/extract_suggest.mjs';

const HELP_TEXT = `Usage:
  cambridge_scraper.mjs suggest --query <word> [--mode <mode>] [--max-results <n>] [--timeout-ms <ms>] [--headless <true|false>]
  cambridge_scraper.mjs define --entry <word> [--mode <mode>] [--max-results <n>] [--timeout-ms <ms>] [--headless <true|false>]

Modes:
  english
  english-chinese-traditional
`;

function clampInteger(value, { fallback, min, max }) {
  const parsed = Number.parseInt(String(value ?? ''), 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, parsed));
}

function parseBoolean(value, fallback = true) {
  const normalized = String(value ?? '').trim().toLowerCase();
  if (!normalized) {
    return fallback;
  }
  if (['1', 'true', 'yes', 'on'].includes(normalized)) {
    return true;
  }
  if (['0', 'false', 'no', 'off'].includes(normalized)) {
    return false;
  }
  return fallback;
}

function normalizeText(value) {
  return String(value ?? '').trim();
}

function parseCli(argv) {
  if (argv.length === 0 || argv[0] === '--help' || argv[0] === '-h') {
    return { help: true };
  }

  const command = argv[0];
  if (command !== 'suggest' && command !== 'define') {
    throw new Error(`invalid command: ${command}`);
  }

  const args = {
    command,
    query: '',
    entry: '',
    mode: process.env.CAMBRIDGE_DICT_MODE || 'english',
    maxResults: process.env.CAMBRIDGE_MAX_RESULTS || '8',
    timeoutMs: process.env.CAMBRIDGE_TIMEOUT_MS || '8000',
    headless: process.env.CAMBRIDGE_HEADLESS || 'true',
    fixtureHtml: '',
  };

  for (let i = 1; i < argv.length; i += 1) {
    const token = argv[i];
    const next = argv[i + 1];

    const consumeValue = (name) => {
      if (!next || next.startsWith('--')) {
        throw new Error(`${name} requires a value`);
      }
      i += 1;
      return next;
    };

    if (token === '--query') {
      args.query = consumeValue('--query');
    } else if (token === '--entry') {
      args.entry = consumeValue('--entry');
    } else if (token === '--mode') {
      args.mode = consumeValue('--mode');
    } else if (token === '--max-results') {
      args.maxResults = consumeValue('--max-results');
    } else if (token === '--timeout-ms') {
      args.timeoutMs = consumeValue('--timeout-ms');
    } else if (token === '--headless') {
      args.headless = consumeValue('--headless');
    } else if (token === '--fixture-html') {
      args.fixtureHtml = consumeValue('--fixture-html');
    } else if (token === '--help' || token === '-h') {
      return { help: true };
    } else {
      throw new Error(`unknown argument: ${token}`);
    }
  }

  args.mode = normalizeMode(args.mode);
  args.maxResults = clampInteger(args.maxResults, { fallback: 8, min: 1, max: 20 });
  args.timeoutMs = clampInteger(args.timeoutMs, { fallback: 8000, min: 1000, max: 30000 });
  args.headless = parseBoolean(args.headless, true);

  if (args.command === 'suggest') {
    args.query = normalizeText(args.query);
    if (!args.query) {
      throw new Error('query must not be empty');
    }
  }

  if (args.command === 'define') {
    args.entry = normalizeText(args.entry);
    if (!args.entry) {
      throw new Error('entry must not be empty');
    }
  }

  return args;
}

function writeJson(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

function fallbackMode() {
  try {
    return normalizeMode(process.env.CAMBRIDGE_DICT_MODE || 'english');
  } catch {
    return 'english';
  }
}

async function loadPageSnapshot({ url, timeoutMs, headless, fixtureHtml }) {
  if (fixtureHtml) {
    return {
      html: await readFile(fixtureHtml, 'utf8'),
      finalUrl: '',
    };
  }

  const browser = await chromium.launch({ headless });
  try {
    const context = await browser.newContext({
      userAgent:
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 13_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36',
    });

    try {
      const page = await context.newPage();
      await page.goto(url, {
        waitUntil: 'domcontentloaded',
        timeout: timeoutMs,
      });
      await page.waitForTimeout(120);
      return {
        html: await page.content(),
        finalUrl: page.url(),
      };
    } finally {
      await context.close();
    }
  } finally {
    await browser.close();
  }
}

async function runSuggest(args) {
  const url = buildSuggestUrl({ query: args.query, mode: args.mode });
  const { html, finalUrl } = await loadPageSnapshot({
    url,
    timeoutMs: args.timeoutMs,
    headless: args.headless,
    fixtureHtml: args.fixtureHtml,
  });

  const barrier = classifyHtmlBarrier(html);
  if (barrier) {
    return {
      ok: false,
      stage: 'suggest',
      mode: args.mode,
      query: args.query,
      error: barrier,
    };
  }

  const items = extractSuggestFromHtml({
    html,
    mode: args.mode,
    maxResults: args.maxResults,
    pageUrl: finalUrl,
  });

  return {
    ok: true,
    stage: 'suggest',
    mode: args.mode,
    query: args.query,
    items,
  };
}

async function runDefine(args) {
  const entry = sanitizeEntry(args.entry);
  const url = buildDefineUrl({ entry, mode: args.mode });
  const { html } = await loadPageSnapshot({
    url,
    timeoutMs: args.timeoutMs,
    headless: args.headless,
    fixtureHtml: args.fixtureHtml,
  });

  const barrier = classifyHtmlBarrier(html);
  if (barrier) {
    return {
      ok: false,
      stage: 'define',
      mode: args.mode,
      query: entry,
      error: barrier,
    };
  }

  const extracted = extractDefineFromHtml({
    html,
    mode: args.mode,
    entry,
  });

  return {
    ok: true,
    stage: 'define',
    mode: args.mode,
    entry: extracted,
  };
}

async function main() {
  let args;
  try {
    args = parseCli(process.argv.slice(2));
  } catch (error) {
    writeJson(
      asStructuredError({
        stage: 'unknown',
        mode: fallbackMode(),
        error,
      }),
    );
    process.exit(2);
  }

  if (args.help) {
    process.stdout.write(HELP_TEXT);
    return;
  }

  try {
    const payload = args.command === 'suggest' ? await runSuggest(args) : await runDefine(args);
    writeJson(payload);
    process.exit(payload.ok ? 0 : 1);
  } catch (error) {
    writeJson(
      asStructuredError({
        stage: args.command,
        mode: args.mode,
        error,
      }),
    );
    process.exit(1);
  }
}

await main();
