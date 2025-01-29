export const IS_PROD = process.env.NODE_ENV === 'production';

export const SENTRY_DSN = IS_PROD
    ? 'https://cb83626ca07d6cf66ca2f901cf53c051@o4508279186718720.ingest.de.sentry.io/4508647247249488'
    : 'https://ba5d6596291f12e88625e02eb942b742@o4508279186718720.ingest.de.sentry.io/4508647248691280';

export const SENTRY_PROJECT_NAME = IS_PROD ? 'iota-wallet-dashboard' : 'iota-wallet-dashboard-dev';
export const SENTRY_ORG_NAME = 'iota-foundation-eu';
