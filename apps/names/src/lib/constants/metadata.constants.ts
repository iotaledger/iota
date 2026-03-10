// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Metadata } from 'next';

import { CONFIG } from '@/config';

const BASE_URL = CONFIG.baseUrl;

const DEFAULT_TITLE = 'IOTA Names';
const DEFAULT_DESCRIPTION = 'Own a unique, human-readable name on IOTA.';
const DEFAULT_IMAGE = '/meta_image.png';

export const DEFAULT_METADATA: Metadata = {
    metadataBase: new URL(BASE_URL),
    title: DEFAULT_TITLE,
    description: DEFAULT_DESCRIPTION,
    openGraph: {
        title: DEFAULT_TITLE,
        description: DEFAULT_DESCRIPTION,
        images: [DEFAULT_IMAGE],
        type: 'website',
    },
    twitter: {
        card: 'summary_large_image',
        title: DEFAULT_TITLE,
        description: DEFAULT_DESCRIPTION,
        images: [DEFAULT_IMAGE],
    },
};
