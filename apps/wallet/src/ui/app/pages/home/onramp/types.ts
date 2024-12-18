// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ComponentProps, type FC } from 'react';

export interface OnrampProvider {
	key: string;
	icon: FC<ComponentProps<'svg'>>;
	name: string;
	checkSupported(): Promise<boolean>;
	getUrl(address: string): Promise<string>;
}
