// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import Admonition from '@theme/Admonition';
import Link from '@docusaurus/Link';

export default {
  '/iota-identity*': <Admonition type='info'>IOTA Identity for Rebased is currently in alpha and may still be subject to significant changes.</Admonition>,
  '/iota-evm*': <Admonition type='info'>IOTA EVM is not available on the IOTA Rebased Tesnet at the moment, please use the <Link href="/iota-evm/getting-started/networks-and-chains">EVM Networks and Chains</Link>.</Admonition>,
};
