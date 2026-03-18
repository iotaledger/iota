// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { Transaction } from '../../src/transactions';

describe('System objects with mutable parameter', () => {
    it('system object without options returns UnresolvedObject', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_system',
            arguments: [tx.object.system()],
        });

        const data = tx.getData();
        const systemInput = data.inputs[0];
        expect(systemInput).toEqual({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x0000000000000000000000000000000000000000000000000000000000000005',
                initialSharedVersion: 1,
            },
        });
    });

    it('system object with mutable: true returns SharedObject', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_system',
            arguments: [tx.object.system({ mutable: true })],
        });

        const data = tx.getData();
        const systemInput = data.inputs[0];
        expect(systemInput).toEqual({
            $kind: 'Object',
            Object: {
                $kind: 'SharedObject',
                SharedObject: {
                    objectId: '0x0000000000000000000000000000000000000000000000000000000000000005',
                    initialSharedVersion: 1,
                    mutable: true,
                },
            },
        });
    });

    it('system object with mutable: false returns SharedObject with mutable: false', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_system',
            arguments: [tx.object.system({ mutable: false })],
        });

        const data = tx.getData();
        const systemInput = data.inputs[0];
        expect(systemInput).toEqual({
            $kind: 'Object',
            Object: {
                $kind: 'SharedObject',
                SharedObject: {
                    objectId: '0x0000000000000000000000000000000000000000000000000000000000000005',
                    initialSharedVersion: 1,
                    mutable: false,
                },
            },
        });
    });

    it('denyList object without options returns UnresolvedObject', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_denylist',
            arguments: [tx.object.denyList()],
        });

        const data = tx.getData();
        const denyListInput = data.inputs[0];
        expect(denyListInput).toEqual({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x0000000000000000000000000000000000000000000000000000000000000403',
            },
        });
    });

    it('denyList object with mutable: true returns SharedObject', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_denylist',
            arguments: [tx.object.denyList({ mutable: true })],
        });

        const data = tx.getData();
        const denyListInput = data.inputs[0];
        expect(denyListInput).toEqual({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x0000000000000000000000000000000000000000000000000000000000000403',
                mutable: true,
            },
        });
    });

    it('denyList object with mutable: false returns SharedObject with mutable: false', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_denylist',
            arguments: [tx.object.denyList({ mutable: false })],
        });

        const data = tx.getData();
        const denyListInput = data.inputs[0];
        expect(denyListInput).toEqual({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x0000000000000000000000000000000000000000000000000000000000000403',
                mutable: false,
            },
        });
    });

    it('clock and random objects remain immutable SharedObject', () => {
        const tx = new Transaction();
        tx.moveCall({
            target: '0x2::example::use_clock_and_random',
            arguments: [tx.object.clock(), tx.object.random()],
        });

        const data = tx.getData();
        const clockInput = data.inputs[0];
        const randomInput = data.inputs[1];

        expect(clockInput).toEqual({
            $kind: 'Object',
            Object: {
                $kind: 'SharedObject',
                SharedObject: {
                    objectId: '0x0000000000000000000000000000000000000000000000000000000000000006',
                    initialSharedVersion: 1,
                    mutable: false,
                },
            },
        });

        expect(randomInput).toEqual({
            $kind: 'UnresolvedObject',
            UnresolvedObject: {
                objectId: '0x0000000000000000000000000000000000000000000000000000000000000008',
                mutable: false,
            },
        });
    });
});
