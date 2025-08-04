// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReceiverInputFormValues } from '@iota/core';

export interface FormDataValues extends ReceiverInputFormValues {
    amount: string;
}
