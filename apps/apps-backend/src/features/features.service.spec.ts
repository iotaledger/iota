// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Test, TestingModule } from '@nestjs/testing';
import { FeaturesService } from './features.service';
import { Feature } from '@iota/core/enums/features.enums';
import * as versionedFeaturesModule from './versioned-features';

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

        it('should return all features when version is undefined', () => {
            const result = service.getStagingFeatures(undefined);
            expect(result.features[Feature.WalletPasskeys]).toBeDefined();
        });
    });

    describe('getProductionFeatures', () => {
        it('should return all features when no version is provided', () => {
            const result = service.getProductionFeatures();
            expect(result.status).toBe(200);
            expect(result.features).toBeDefined();
            expect(result.dateUpdated).toBeDefined();
        });

        it('should return all features when version is undefined', () => {
            const result = service.getProductionFeatures(undefined);
            expect(result.features[Feature.WalletPasskeys]).toBeDefined();
        });
    });

    describe('applyVersionFilter', () => {
        const mockFeatures = {
            'feature-a': { defaultValue: true },
            'feature-b': { defaultValue: false },
            'feature-c': { defaultValue: 'hello' },
        };

        it('should return all features when no version is provided', () => {
            const result = service.applyVersionFilter(mockFeatures, 'staging');
            expect(result).toEqual(mockFeatures);
        });

        it('should return all features when version is an empty string', () => {
            const result = service.applyVersionFilter(mockFeatures, 'staging', '');
            expect(result).toEqual(mockFeatures);
        });

        it('should return all features when version is invalid', () => {
            const result = service.applyVersionFilter(mockFeatures, 'staging', 'not-a-version');
            expect(result).toEqual(mockFeatures);
        });

        it('should return all features when no versioned rules are defined', () => {
            const result = service.applyVersionFilter(mockFeatures, 'staging', '1.0.0');
            expect(result).toEqual(mockFeatures);
        });

        describe('with versioned feature rules', () => {
            const originalVersionedFeatures = { ...versionedFeaturesModule.VERSIONED_FEATURES };

            beforeEach(() => {
                // Set up a test rule: feature-b requires version >= 2.0.0
                (versionedFeaturesModule.VERSIONED_FEATURES as any)['feature-b'] = {
                    minVersion: '2.0.0',
                    staging: 'staging-override',
                    production: 'production-override',
                };
            });

            afterEach(() => {
                // Restore original state
                for (const key of Object.keys(versionedFeaturesModule.VERSIONED_FEATURES)) {
                    delete (versionedFeaturesModule.VERSIONED_FEATURES as any)[key];
                }
                Object.assign(
                    versionedFeaturesModule.VERSIONED_FEATURES,
                    originalVersionedFeatures,
                );
            });

            it('should exclude features when version is below minVersion', () => {
                const result = service.applyVersionFilter(mockFeatures, 'staging', '1.9.9');
                expect(result['feature-a']).toEqual({ defaultValue: true });
                expect(result['feature-b']).toBeUndefined();
                expect(result['feature-c']).toEqual({ defaultValue: 'hello' });
            });

            it('should include features with override when version meets minVersion', () => {
                const result = service.applyVersionFilter(mockFeatures, 'staging', '2.0.0');
                expect(result['feature-a']).toEqual({ defaultValue: true });
                expect(result['feature-b']).toEqual({ defaultValue: 'staging-override' });
                expect(result['feature-c']).toEqual({ defaultValue: 'hello' });
            });

            it('should include features with override when version exceeds minVersion', () => {
                const result = service.applyVersionFilter(mockFeatures, 'staging', '3.0.0');
                expect(result['feature-b']).toEqual({ defaultValue: 'staging-override' });
            });

            it('should use the correct environment override', () => {
                const stagingResult = service.applyVersionFilter(mockFeatures, 'staging', '2.0.0');
                expect(stagingResult['feature-b']).toEqual({
                    defaultValue: 'staging-override',
                });

                const productionResult = service.applyVersionFilter(
                    mockFeatures,
                    'production',
                    '2.0.0',
                );
                expect(productionResult['feature-b']).toEqual({
                    defaultValue: 'production-override',
                });
            });

            it('should keep the original value when no environment override is defined', () => {
                // Remove the staging override
                delete (versionedFeaturesModule.VERSIONED_FEATURES as any)['feature-b'].staging;

                const result = service.applyVersionFilter(mockFeatures, 'staging', '2.0.0');
                expect(result['feature-b']).toEqual({ defaultValue: false });
            });
        });
    });
});
