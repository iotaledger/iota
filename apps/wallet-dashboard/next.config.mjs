// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/** @type {import('next').NextConfig} */
const nextConfig = {
    async redirects() {
        return [
            {
                source: '/',
                destination: '/dashboard/home',
                permanent: true,
            },
            {
                source: '/dashboard',
                destination: '/dashboard/home',
                permanent: true,
            },
        ];
    },
};

export default nextConfig;
