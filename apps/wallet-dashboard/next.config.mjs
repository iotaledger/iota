// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
const NEXT_PUBLIC_DASHBOARD_DEV = execSync('git rev-parse HEAD').toString().trim().toString();


/** @type {import('next').NextConfig} */
const nextConfig = {
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
        NEXT_PUBLIC_DASHBOARD_DEV
    }
};

export default nextConfig;
