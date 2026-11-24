// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A TransactionDenyRulesUpdate transaction cannot be built before the
// TransactionDenyRules object has been created at the end of an epoch.

//# init --simulator

//# update-deny-rules --added-addresses 0xAA
