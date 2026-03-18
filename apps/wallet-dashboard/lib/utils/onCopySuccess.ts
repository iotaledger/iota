// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';
import { ampli } from '@/lib/utils/analytics/ampli';

export interface CopySuccessOptions {
    message: string;
    analyticType?: string;
}

export function getCopySuccessHandler(options: CopySuccessOptions): () => void {
    return () => {
        toast(options.message);

        if (options.analyticType) {
            ampli.elementCopied({ type: options.analyticType });
        }
    };
}
