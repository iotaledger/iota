// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState } from 'react';

import { Navbar } from '@/components/layout';
import { TestModeBanner } from '@/components/TestModeBanner';

export function StickyHeader() {
    const [isBannerVisible, setIsBannerVisible] = useState(true);
    return (
        <div className="fixed top-0 left-0 w-full z-50 transition-all duration-300">
            <TestModeBanner
                isBannerVisible={isBannerVisible}
                onDismiss={() => setIsBannerVisible(false)}
            />
            <Navbar isBannerVisible={isBannerVisible} />
        </div>
    );
}
