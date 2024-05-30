// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { Text } from '../shared/text';

export function SectionHeader({ title }: { title: string }) {
	return (
		<div className="flex items-center justify-center gap-3">
			<div className="flex h-px flex-1 flex-shrink-0 bg-gray-45" />
			<Text variant="caption" weight="semibold" color="steel">
				{title}
			</Text>
			<div className="flex h-px flex-1 flex-shrink-0 bg-gray-45" />
		</div>
	);
}
