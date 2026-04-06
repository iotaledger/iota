// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Metadata } from 'next';

const PRODUCTION_BASE_URL = 'wallet-dashboard.iota.org';
const VERCEL_URL = process.env.VERCEL_URL;

const METADATA_INFO = {
    title: 'IOTA Wallet Dashboard',
    description: 'IOTA Wallet Dashboard - Connecting you to the decentralized web and IOTA network',
    image: '/metadata-image.png',
    metadataBase: new URL(`https://${VERCEL_URL || PRODUCTION_BASE_URL}`),
};

export const METADATA: Metadata = {
    metadataBase: METADATA_INFO.metadataBase,
    title: METADATA_INFO.title,
    description: METADATA_INFO.description,
    openGraph: {
        title: METADATA_INFO.title,
        description: METADATA_INFO.description,
        images: [METADATA_INFO.image],
    },
    twitter: {
        title: METADATA_INFO.title,
        description: METADATA_INFO.description,
        images: [METADATA_INFO.image],
    },
};
