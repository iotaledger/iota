// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { Collapsible } from '_src/ui/app/shared/collapse';
import { type ReactNode } from 'react';

type Props = {
	title: string;
	defaultOpen?: boolean;
	children: ReactNode;
};

export function TokenList({ title, defaultOpen, children }: Props) {
	return (
		<div className="flex w-full flex-shrink-0 flex-col justify-start">
			<Collapsible title={title} defaultOpen={defaultOpen}>
				{children}
			</Collapsible>
		</div>
	);
}
