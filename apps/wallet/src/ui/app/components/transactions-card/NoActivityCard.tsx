// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_src/ui/app/shared/text';
import { TransferObject16 } from '@iota/icons';

export type NoActivityCardType = {
	message: string;
};

export function NoActivityCard({ message }: NoActivityCardType) {
	return (
		<div className="flex flex-col gap-4 justify-center items-center text-center h-full px-10">
			<TransferObject16 className="text-gray-45 text-3xl" />
			<Text variant="pBody" weight="medium" color="steel">
				{message}
			</Text>
		</div>
	);
}
