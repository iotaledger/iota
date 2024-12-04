import type { Config } from 'jest';

const config: Config = {
    clearMocks: true,
    coverageProvider: 'v8',
    transform: {
        '^.+\\.(ts|tsx)$': 'ts-jest',
    },
    moduleNameMapper: {
        '^@iota/core/constants/(.*)$': '<rootDir>/../core/src/constants/$1',
    },
};

export default config;
