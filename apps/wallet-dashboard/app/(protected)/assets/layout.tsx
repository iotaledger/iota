// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { HiddenAssetsProvider } from '@iota/core';

export default function AssetsLayout({
    children,
}: Readonly<{
    children: React.ReactNode;
}>) {
    return <HiddenAssetsProvider>{children}</HiddenAssetsProvider>;
}
