// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/dapp-kit/dist/index.css';
import './globals.css';
import { Inter } from 'next/font/google';
import { Metadata } from 'next';
import { AppProviders } from '@/providers';
import { FontLinks } from '@/components/FontLinks';
import { ConnectionGuard } from '@/components/connection-guard';
import { Amplitude } from '@/components/Amplitude';

const inter = Inter({ subsets: ['latin'] });

const METADATA_INFO = {
    title: 'IOTA Wallet Dashboard',
    description: 'IOTA Wallet Dashboard - Connecting you to the decentralized web and IOTA network',
    image: '/metadata-image.png',
};

export const metadata: Metadata = {
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

const legacyBannerScript = `
(() => {
  try {
    const ua = navigator.userAgent;
    const version = (re) => +(ua.match(re)?.[1] || 999);

    const isLegacy =
      version(/Chrome\\/(\\d+)/) < 200 ||
      version(/Firefox\\/(\\d+)/) < 94 ||
      (/Safari/.test(ua) && !/Chrome/.test(ua) && parseFloat(ua.match(/Version\\/(\\d+\\.\\d+)/)?.[1] || 99) < 15.4) ||
      version(/Edg\\/(\\d+)/) < 98 ||
      version(/OPR\\/(\\d+)/) < 84 ||
      typeof structuredClone !== 'function';

    if (!isLegacy) return;

    const banner = Object.assign(document.createElement('div'), {
      textContent: 'Your browser version is outdated. Please update it to the latest version.'
    });
    banner.style.cssText =
      'position:fixed;top:1rem;right:1rem;z-index:99999;background:#facc15;color:#1a1a1a;font:500 0.875rem/1.25rem system-ui,sans-serif;padding:0.75rem 1rem;border-radius:0.5rem;box-shadow:0 4px 10px rgba(0,0,0,0.15);transition:opacity .5s ease;';

    document.addEventListener('DOMContentLoaded', () => {
      document.body.appendChild(banner);
      // Oculta el banner después de 6 segundos con una pequeña animación
      setTimeout(() => {
        banner.style.opacity = '0';
        setTimeout(() => banner.remove(), 500);
      }, 6000);
    });
  } catch {
    const fallback = document.createElement('div');
    fallback.textContent = 'Your browser is too old to display this page.';
    fallback.style.cssText =
      'position:fixed;top:1rem;right:1rem;z-index:99999;background:#f87171;color:white;padding:0.75rem 1rem;border-radius:0.5rem;font-family:sans-serif;';
    document.addEventListener('DOMContentLoaded', () => document.body.appendChild(fallback));
  }
})();
`;

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    return (
        <html lang="en">
            <head>
                <script dangerouslySetInnerHTML={{ __html: legacyBannerScript }} />
            </head>
            <body className={inter.className}>
                <AppProviders>
                    <FontLinks />
                    <Amplitude />
                    <ConnectionGuard>{children}</ConnectionGuard>
                </AppProviders>
            </body>
        </html>
    );
}
