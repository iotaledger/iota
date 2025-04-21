// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { addressToStardustBech32, StardustAddressType } from '../legacy';

describe('addressToStardustBech32', () => {
    it('address type ed25519: correctly converts a valid address (with 0x) to bech32 format', () => {
        const hexAddress = '0xecf4d91434698a8ee23849c686a4b73998653582a319eb62ed70aee87031ff8e';
        const bech32Address = addressToStardustBech32(hexAddress);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.charAt(5)).toEqual('q');
        expect(bech32Address).toEqual(
            'iota1qrk0fkg5x35c4rhz8pyudp4ykuuesef4s233n6mza4c2a6rsx8lcu4zs3qz',
        );
    });
    it('address type ed25519: correctly converts a valid address(without 0x) to bech32 format', () => {
        const hexAddress = 'ecf4d91434698a8ee23849c686a4b73998653582a319eb62ed70aee87031ff8e';
        const bech32Address = addressToStardustBech32(hexAddress);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.charAt(5)).toEqual('q');
        expect(bech32Address).toEqual(
            'iota1qrk0fkg5x35c4rhz8pyudp4ykuuesef4s233n6mza4c2a6rsx8lcu4zs3qz',
        );
    });
    it('address type alias: correctly converts a valid address (with 0x) to bech32 format', () => {
        const hexAddress = '0xaa309a0bdc7a17ad8013fa6f033c120206dce928d07058880fc72a3fc594518a';
        const bech32Address = addressToStardustBech32(hexAddress, StardustAddressType.Alias);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.charAt(5)).toEqual('p');
        expect(bech32Address).toEqual(
            'iota1pz4rpxstm3ap0tvqz0ax7qeuzgpqdh8f9rg8qkygplrj5079j3gc5zf2mm2',
        );
    });
    it('address type alias: correctly converts a valid address(without 0x) to bech32 format', () => {
        const hexAddress = 'aa309a0bdc7a17ad8013fa6f033c120206dce928d07058880fc72a3fc594518a';
        const bech32Address = addressToStardustBech32(hexAddress, StardustAddressType.Alias);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.charAt(5)).toEqual('p');
        expect(bech32Address).toEqual(
            'iota1pz4rpxstm3ap0tvqz0ax7qeuzgpqdh8f9rg8qkygplrj5079j3gc5zf2mm2',
        );
    });
    it('address type nft: correctly converts a valid address (with 0x) to bech32 format', () => {
        const hexAddress = '0x83b9856dc0b8ef7465551a9eac20b493abe312b42e87a4b196721e04c34bb9ce';
        const bech32Address = addressToStardustBech32(hexAddress, StardustAddressType.Nft);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.charAt(5)).toEqual('z');
        expect(bech32Address).toEqual(
            'iota1zzpmnptdczuw7ar925dfatpqkjf6hccjkshg0f93jeepupxrfwuuu4vz09l',
        );
    });
    it('address type nft: correctly converts a valid address(without 0x) to bech32 format', () => {
        const hexAddress = '83b9856dc0b8ef7465551a9eac20b493abe312b42e87a4b196721e04c34bb9ce';
        const bech32Address = addressToStardustBech32(hexAddress, StardustAddressType.Nft);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.charAt(5)).toEqual('z');
        expect(bech32Address).toEqual(
            'iota1zzpmnptdczuw7ar925dfatpqkjf6hccjkshg0f93jeepupxrfwuuu4vz09l',
        );
    });
});
