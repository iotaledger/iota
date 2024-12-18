// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '../shared/text';

export function SectionHeader({ title }: { title: string }) {
	return (
		<div className="flex gap-3 items-center justify-center">
			<div className="h-px bg-gray-45 flex flex-1 flex-shrink-0" />
			<Text variant="caption" weight="semibold" color="steel">
				{title}
			</Text>
			<div className="h-px bg-gray-45 flex flex-1 flex-shrink-0" />
		</div>
	);
}
