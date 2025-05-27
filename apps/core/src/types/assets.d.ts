// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

declare module '*.svg' {
    import { type FC, type ComponentProps } from 'react';
    const component: FC<ComponentProps<'svg'>>;
    export default component;
}
