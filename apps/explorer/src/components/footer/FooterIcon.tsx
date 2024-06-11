// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PropsWithChildren } from 'react';

export function FooterIcon({ children }: PropsWithChildren): JSX.Element {
    return <div className="flex items-center text-steel-darker">{children}</div>;
}
