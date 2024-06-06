// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as assert from 'assert';
import * as Mocha from 'mocha';
import * as vscode from 'vscode';

Mocha.iotate('ext', () => {
    Mocha.test('ext_exists', () => {
        const ext = vscode.extensions.getExtension('move.move-analyzer');
        assert.ok(ext);
    });
});
