// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The TransactionDenyRulesCreate end-of-epoch transaction kind creates the
// shared TransactionDenyRules object at its reserved address 0xDE9, with all
// deny tables empty and all switches off.

//# init --simulator --deny-rule-governance true

//# view-object 0xDE9

//# advance-epoch --create-deny-rules-object

//# view-object 0xDE9

//# view-object 2,0
