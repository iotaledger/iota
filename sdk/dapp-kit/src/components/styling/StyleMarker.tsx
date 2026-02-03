// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Slot } from '@radix-ui/react-slot';
import type { ComponentPropsWithoutRef, ComponentRef, ReactNode } from 'react';

import { styleDataAttribute } from '../../constants/styleDataAttribute.js';

import './StyleMarker.css.js';

type StyleMarkerProps = {
    children: ReactNode;
    ref?: React.Ref<ComponentRef<typeof Slot>>;
} & ComponentPropsWithoutRef<typeof Slot>;

export const StyleMarker = ({ children, ref, ...props }: StyleMarkerProps) => (
    <Slot ref={ref} {...props} {...styleDataAttribute}>
        {children}
    </Slot>
);
StyleMarker.displayName = 'StyleMarker';
