// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRouter } from 'next/router';

const config = {
    logo: <span>Iota TypeScript Docs</span>,
    project: {
        link: 'https://github.com/iotaledger/iota/tree/main/sdk/',
    },
    chat: {
        link: 'https://discord.com/invite/Iota',
    },
    docsRepositoryBase: 'https://github.com/iotaledger/iota/tree/main/sdk/docs',
    footer: {
        text: `Copyright © ${new Date().getFullYear()}, Mysten Labs, Inc.`,
    },
    head: (
        <>
            <meta
                name="google-site-verification"
                content="T-2HWJAKh8s63o9KFxCFXg5MON_NGLJG76KJzr_Hp0A"
            />
            <meta httpEquiv="Content-Language" content="en" />
        </>
    ),
    useNextSeoProps() {
        const { asPath } = useRouter();

        return {
            titleTemplate: asPath !== '/' ? '%s | Iota TypeScript Docs' : 'Iota TypeScript Docs',
            description:
                'Iota TypeScript Documentation. Discover the power of Iota through examples, guides, and concepts.',
            openGraph: {
                title: 'Iota TypeScript Docs',
                description:
                    'Iota TypeScript Documentation. Discover the power of Iota through examples, guides, and concepts.',
                site_name: 'Iota TypeScript Docs',
            },
            additionalMetaTags: [
                { content: 'Iota TypeScript Docs', name: 'apple-mobile-web-app-title' },
            ],
            twitter: {
                card: 'summary_large_image',
                site: '@Mysten_Labs',
            },
        };
    },
};

export default config;
