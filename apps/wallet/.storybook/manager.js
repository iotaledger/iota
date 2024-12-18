// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { addons } from '@storybook/addons';
import { themes } from '@storybook/theming';

// Force the theme to light, as our components do not suppor theming
addons.setConfig({
	theme: themes.light,
});
