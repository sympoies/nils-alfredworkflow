export const ERROR_CODES = Object.freeze({
  ANTI_BOT: 'anti_bot',
  COOKIE_WALL: 'cookie_wall',
  TIMEOUT: 'timeout',
  NETWORK: 'network',
  PARSE_ERROR: 'parse_error',
  INVALID_ARGS: 'invalid_args',
  UNKNOWN: 'unknown',
});

function normalizeText(value) {
  return String(value ?? '').toLowerCase();
}

function containsAny(text, patterns) {
  return patterns.some((pattern) => text.includes(pattern));
}

function hasDictionaryContent(lowerHtml) {
  if (!lowerHtml) {
    return false;
  }

  const contentMarkers = [
    'class="entry-body',
    "class='entry-body",
    'class="def ddef_d',
    "class='def ddef_d",
    'class="def-body',
    "class='def-body",
    'class="headword',
    "class='headword",
    'link rel="canonical" href="https://dictionary.cambridge.org/dictionary/',
    'property="og:url" content="https://dictionary.cambridge.org/dictionary/',
    'search suggestions for',
    'did you spell it correctly',
    'alternative spellings in the cambridge',
    'we have these words with similar spellings or pronunciations',
    'https://dictionary.cambridge.org/spellcheck/',
  ];

  return containsAny(lowerHtml, contentMarkers);
}

function createClassification({ code, message, hint, retriable }) {
  return {
    code,
    message,
    hint,
    retriable: Boolean(retriable),
  };
}

export function classifyHtmlBarrier(html) {
  const lower = normalizeText(html);
  if (!lower) {
    return null;
  }

  if (hasDictionaryContent(lower)) {
    return null;
  }

  const antiBotSignals = [
    'attention required! | cloudflare',
    'verify you are human',
    'cf-challenge',
    'cf-turnstile',
    '/cdn-cgi/challenge-platform',
    'why do i have to complete a captcha',
    'automated requests',
  ];

  if (containsAny(lower, antiBotSignals)) {
    return createClassification({
      code: ERROR_CODES.ANTI_BOT,
      message: 'cambridge returned an anti-bot challenge page',
      hint: 'retry later or open Cambridge Dictionary directly in your browser',
      retriable: true,
    });
  }

  if (
    lower.includes('cookie') &&
    (lower.includes('consent') || lower.includes('preferences') || lower.includes('enable'))
  ) {
    return createClassification({
      code: ERROR_CODES.COOKIE_WALL,
      message: 'cambridge requires cookie consent before content is visible',
      hint: 'open Cambridge Dictionary once in your browser and accept cookies',
      retriable: true,
    });
  }

  return null;
}

export function classifyRuntimeError(error) {
  const message = normalizeText(error?.message || error);

  if (!message) {
    return createClassification({
      code: ERROR_CODES.UNKNOWN,
      message: 'unknown scraper failure',
      hint: 'retry later',
      retriable: false,
    });
  }

  if (message.includes('timeout') || message.includes('timed out')) {
    return createClassification({
      code: ERROR_CODES.TIMEOUT,
      message: 'scraper timed out while waiting for page content',
      hint: 'increase timeout-ms or retry with a shorter query',
      retriable: true,
    });
  }

  if (
    message.includes('net::err_') ||
    message.includes('name_not_resolved') ||
    message.includes('connection') ||
    message.includes('dns') ||
    message.includes('econnreset') ||
    message.includes('socket hang up')
  ) {
    return createClassification({
      code: ERROR_CODES.NETWORK,
      message: 'network failure while fetching Cambridge page',
      hint: 'check network connectivity and retry',
      retriable: true,
    });
  }

  if (message.includes('entry must not be empty') || message.includes('query must not be empty') || message.includes('invalid mode')) {
    return createClassification({
      code: ERROR_CODES.INVALID_ARGS,
      message: String(error?.message || error),
      hint: 'check required CLI arguments and mode value',
      retriable: false,
    });
  }

  if (message.includes('extract') || message.includes('parse')) {
    return createClassification({
      code: ERROR_CODES.PARSE_ERROR,
      message: 'failed to extract dictionary data from page',
      hint: 'Cambridge DOM may have changed; update selectors and retry',
      retriable: false,
    });
  }

  return createClassification({
    code: ERROR_CODES.UNKNOWN,
    message: String(error?.message || error),
    hint: 'retry later',
    retriable: false,
  });
}

export function classifyScraperError({ error, html }) {
  const htmlClassified = classifyHtmlBarrier(html);
  if (htmlClassified) {
    return htmlClassified;
  }
  return classifyRuntimeError(error);
}

export function asStructuredError({ stage, mode, error, html }) {
  const classified = classifyScraperError({ error, html });
  return {
    ok: false,
    stage,
    mode,
    error: classified,
  };
}
