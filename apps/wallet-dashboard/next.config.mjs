// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SENTRY_ORG_NAME, SENTRY_PROJECT_NAME } from './sentry.common.config.mjs';
import { withSentryConfig } from '@sentry/nextjs';
import { execSync } from 'child_process';
const NEXT_PUBLIC_DASHBOARD_REV = execSync('git rev-parse HEAD').toString().trim().toString();
const NEXT_PUBLIC_BUILD_ENV = process.env.BUILD_ENV;
const APPS_BACKEND = process.env.APPS_BACKEND;

/** @type {import('next').NextConfig} */
const nextConfig = {
    transpilePackages: ['@iota/core'],
    async redirects() {
        return [
            {
                source: '/dashboard',
                destination: '/home',
                permanent: true,
            },
        ];
    },
    images: {
        // Remove this domain when fetching data
        domains: ['d315pvdvxi2gex.cloudfront.net'],
    },
    env: {
        NEXT_PUBLIC_DASHBOARD_REV,
        NEXT_PUBLIC_BUILD_ENV,
        APPS_BACKEND,
    },
    webpack(config) {
        const fileLoaderRule = config.module.rules.find((rule) => rule.test?.test?.('.svg'));
        if (fileLoaderRule) {
            fileLoaderRule.exclude = /\.svg$/i;
        }

        config.module.rules.push({
            test: /\.svg$/i,
            issuer: /\.[jt]sx?$/,
            use: ['@svgr/webpack'],
        });

        return config;
    },
};

export default withSentryConfig(nextConfig, {
    // For all available options, see:
    // https://docs.sentry.io/platforms/javascript/guides/nextjs/configuration/build/

    org: SENTRY_ORG_NAME,
    project: SENTRY_PROJECT_NAME,

    // Only print logs for uploading source maps in CI
    silent: !process.env.CI,

    // Upload a larger set of source maps for prettier stack traces (increases build time)
    widenClientFileUpload: true,

    // Uncomment to route browser requests to Sentry through a Next.js rewrite to circumvent ad-blockers.
    // This can increase your server load as well as your hosting bill.
    // Note: Check that the configured route will not match with your Next.js middleware, otherwise reporting of client-
    // side errors will fail.
    // tunnelRoute: "/monitoring",

    // Automatically tree-shake Sentry logger statements to reduce bundle size
    bundleSizeOptimizations: {
        excludeDebugStatements: true,
    },

    webpack: {
        // Enables automatic instrumentation of Vercel Cron Monitors. (Does not yet work with App Router route handlers.)
        // See the following for more information:
        // https://docs.sentry.io/product/crons/
        // https://vercel.com/docs/cron-jobs
        automaticVercelMonitors: false,
    },
});
