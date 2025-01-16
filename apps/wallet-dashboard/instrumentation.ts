import * as Sentry from '@sentry/nextjs';

export async function register() {
    // Only client is needed
}

export const onRequestError = Sentry.captureRequestError;
export const captureException = Sentry.captureException;
