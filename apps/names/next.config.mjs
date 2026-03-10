// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import nextMdx from '@next/mdx';
import { withSentryConfig } from '@sentry/nextjs';

import { SENTRY_ORG_NAME, SENTRY_PROJECT_NAME } from './sentry.common.config.mjs';

let NEXT_PUBLIC_IOTA_NAMES_REV = 'development';
const NEXT_PUBLIC_BUILD_ENV = process.env.BUILD_ENV;
const NEXT_PUBLIC_AMPLITUDE_ENABLED =
    process.env.NEXT_PUBLIC_AMPLITUDE_ENABLED || process.env.AMPLITUDE_ENABLED;

try {
    if (process.env.VERCEL_GIT_COMMIT_SHA) {
        NEXT_PUBLIC_IOTA_NAMES_REV = process.env.VERCEL_GIT_COMMIT_SHA;
    } else {
        NEXT_PUBLIC_IOTA_NAMES_REV = execSync('git rev-parse HEAD').toString().trim();
    }
} catch (error) {
    console.warn('Could not get git revision, using default');
}

const withMDX = nextMdx();

/** @type {import('next').NextConfig} */
const nextConfig = withMDX({
    env: {
        NEXT_PUBLIC_AMPLITUDE_ENABLED,
        NEXT_PUBLIC_IOTA_NAMES_REV,
        NEXT_PUBLIC_BUILD_ENV,
    },
    experimental: {
        mdxRs: true,
    },
    turbopack: {
        resolveExtensions: ['.mdx', '.tsx', '.ts', '.jsx', '.js', '.mjs', '.json'],
    },
    async redirects() {
        return [
            // Redirect auctions page to home (feature hidden from UI)
            {
                source: '/auctions',
                destination: '/',
                permanent: false,
            },
            {
                source: '/((?!terms-of-service$).*)', // Dont redirect if the path is already /terms-of-service
                has: [
                    {
                        type: 'query',
                        key: 'modal',
                        value: 'terms_conditions',
                    },
                ],
                destination: '/terms-of-service',
                permanent: true,
            },
            {
                source: '/((?!privacy-policy$).*)', // Dont redirect if the path is already /privacy-policy
                has: [
                    {
                        type: 'query',
                        key: 'modal',
                        value: 'privacy_policy',
                    },
                ],
                destination: '/privacy-policy',
                permanent: true,
            },
        ];
    },
});

export default withSentryConfig(nextConfig, {
    // For all available options, see:
    // https://www.npmjs.com/package/@sentry/webpack-plugin#options

    org: SENTRY_ORG_NAME,
    project: SENTRY_PROJECT_NAME,

    // Only print logs for uploading source maps in CI
    silent: !process.env.CI,

    // For all available options, see:
    // https://docs.sentry.io/platforms/javascript/guides/nextjs/manual-setup/

    // Upload a larger set of source maps for prettier stack traces (increases build time)
    widenClientFileUpload: true,

    // Uncomment to route browser requests to Sentry through a Next.js rewrite to circumvent ad-blockers.
    // This can increase your server load as well as your hosting bill.
    // Note: Check that the configured route will not match with your Next.js middleware, otherwise reporting of client-
    // side errors will fail.
    // tunnelRoute: "/monitoring",

    // Hides source maps from generated client bundles
    hideSourceMaps: false,

    // Automatically tree-shake Sentry logger statements to reduce bundle size
    disableLogger: true,

    // Enables automatic instrumentation of Vercel Cron Monitors. (Does not yet work with App Router route handlers.)
    // See the following for more information:
    // https://docs.sentry.io/product/crons/
    // https://vercel.com/docs/cron-jobs
    automaticVercelMonitors: false,
});
