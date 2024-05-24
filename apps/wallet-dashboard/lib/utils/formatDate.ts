// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export default function formatDate(timeStamp: number): string {
	const date = new Date(timeStamp);
	if (!(date instanceof Date)) return '';
	return new Intl.DateTimeFormat('en-US').format(date);
}
