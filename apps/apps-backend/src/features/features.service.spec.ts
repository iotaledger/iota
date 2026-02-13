// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Test, TestingModule } from '@nestjs/testing';
import { FeaturesService } from './features.service';
import { Feature } from '@iota/core/enums/features.enums';

describe('FeaturesService', () => {
    let service: FeaturesService;

    beforeEach(async () => {
        const module: TestingModule = await Test.createTestingModule({
            providers: [FeaturesService],
        }).compile();

        service = module.get<FeaturesService>(FeaturesService);
    });

    it('should be defined', () => {
        expect(service).toBeDefined();
    });

    describe('getStagingFeatures', () => {
        it('should return all features when no version is provided', () => {
            const result = service.getStagingFeatures();
            expect(result.status).toBe(200);
            expect(result.features).toBeDefined();
            expect(result.dateUpdated).toBeDefined();
            expect(result.features[Feature.WalletPasskeys]).toBeDefined();
        });

        it('should not expose minVersion in the response', () => {
            const result = service.getStagingFeatures('99.0.0');
            for (const entry of Object.values(result.features)) {
                expect(entry).not.toHaveProperty('minVersion');
            }
        });
    });

    describe('getProductionFeatures', () => {
        it('should return all features when no version is provided', () => {
            const result = service.getProductionFeatures();
            expect(result.status).toBe(200);
            expect(result.features).toBeDefined();
            expect(result.dateUpdated).toBeDefined();
        });

        it('should exclude versioned features when version is below minVersion', () => {
            const result = service.getProductionFeatures('0.1.0');
            expect(result.features[Feature.WalletPasskeys]).toBeUndefined();
        });

        it('should include versioned features when version meets minVersion', () => {
            const result = service.getProductionFeatures('1.5.0');
            expect(result.features[Feature.WalletPasskeys]).toBeDefined();
        });

        it('should include all features when no version is sent (backward compat)', () => {
            const result = service.getProductionFeatures();
            expect(result.features[Feature.WalletPasskeys]).toBeDefined();
        });
    });

    describe('applyVersionFilter', () => {
        const mockFeatures = {
            'feature-a': { defaultValue: true },
            'feature-b': { defaultValue: false, minVersion: '2.0.0' },
            'feature-c': { defaultValue: 'hello' },
        };

        it('should return all features when no version is provided', () => {
            const result = service.applyVersionFilter(mockFeatures);
            expect(result).toEqual(mockFeatures);
        });

        it('should return all features when version is an empty string', () => {
            const result = service.applyVersionFilter(mockFeatures, '');
            expect(result).toEqual(mockFeatures);
        });

        it('should return all features when version is invalid', () => {
            const result = service.applyVersionFilter(mockFeatures, 'not-a-version');
            expect(result).toEqual(mockFeatures);
        });

        it('should exclude features when version is below minVersion', () => {
            const result = service.applyVersionFilter(mockFeatures, '1.9.9');
            expect(result['feature-a']).toEqual({ defaultValue: true });
            expect(result['feature-b']).toBeUndefined();
            expect(result['feature-c']).toEqual({ defaultValue: 'hello' });
        });

        it('should include features when version meets minVersion', () => {
            const result = service.applyVersionFilter(mockFeatures, '2.0.0');
            expect(result['feature-a']).toEqual({ defaultValue: true });
            expect(result['feature-b']).toEqual({ defaultValue: false, minVersion: '2.0.0' });
            expect(result['feature-c']).toEqual({ defaultValue: 'hello' });
        });

        it('should include features when version exceeds minVersion', () => {
            const result = service.applyVersionFilter(mockFeatures, '3.0.0');
            expect(result['feature-b']).toEqual({ defaultValue: false, minVersion: '2.0.0' });
        });

        it('should always include features without minVersion', () => {
            const result = service.applyVersionFilter(mockFeatures, '0.0.1');
            expect(result['feature-a']).toEqual({ defaultValue: true });
            expect(result['feature-c']).toEqual({ defaultValue: 'hello' });
        });
    });
});
