// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { uiKitStaticPreset } from '@iota/apps-ui-kit';

export default {
    presets: [uiKitStaticPreset],
    content: ['./src/**/*.{js,jsx,ts,tsx}', './node_modules/@iota/apps-ui-kit/dist/**/*.js'],
};
