// Copyright (c) 2024 IOTA Stiftung

import type { ReceiverInputFormValues } from '@iota/core';

// SPDX-License-Identifier: Apache-2.0
export interface FormDataValues extends ReceiverInputFormValues {
    amount: string;
}
