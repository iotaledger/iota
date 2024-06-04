// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const licenseCheck = require('./rules/license-check.rule');

module.exports = {
    rules: {
        'license-check': licenseCheck,
    },
};
