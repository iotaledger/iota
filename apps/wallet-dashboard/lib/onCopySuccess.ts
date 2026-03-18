// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';
import { ampli } from '@/lib/utils/analytics/ampli';

export interface CopySuccessOptions {
    successMessage: string;
    analyticType: string;
}

export function onCopySuccess(options: CopySuccessOptions): () => void {
    return () => {
        toast(options.successMessage);
        ampli.elementCopied({ type: options.analyticType });
    };
}
