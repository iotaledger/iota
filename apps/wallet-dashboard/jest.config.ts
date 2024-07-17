import type { Config } from 'jest';

const config: Config = {
    clearMocks: true,
    coverageProvider: 'v8',
    transform: {
        '^.+\\.(ts|tsx)$': 'ts-jest',
    },
};

export default config;
