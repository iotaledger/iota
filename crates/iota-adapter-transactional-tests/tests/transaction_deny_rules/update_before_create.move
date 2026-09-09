// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A TransactionDenyRulesUpdate transaction cannot be built before the
// TransactionDenyRules object has been created at the end of an epoch.

//# init --simulator --deny-rule-governance true

//# update-deny-rules --added-addresses 0xAA
