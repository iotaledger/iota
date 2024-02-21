// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { injectDappInterface } from './interface-inject';
import { setupMessagesProxy } from './messages-proxy';

injectDappInterface();
setupMessagesProxy();
