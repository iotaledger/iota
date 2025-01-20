// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { injectDappInterface } from './interfaceInject';
import { setupMessagesProxy } from './messagesProxy';

injectDappInterface();
setupMessagesProxy();
