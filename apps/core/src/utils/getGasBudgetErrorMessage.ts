// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GAS_BUDGET_ERROR_MESSAGES } from '../constants';

export function getGasBudgetErrorMessage(error: Error): string | undefined {
    for (const [errorId, errorMessage] of Object.entries(GAS_BUDGET_ERROR_MESSAGES)) {
        if (error.message.includes(errorId)) {
            return errorMessage;
        }
    }
}
