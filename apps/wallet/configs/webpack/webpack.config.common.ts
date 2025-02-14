// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { exec } from 'child_process';
import { resolve } from 'path';
import { randomBytes } from '@noble/hashes/utils';
import SentryWebpackPlugin from '@sentry/webpack-plugin';
import CopyPlugin from 'copy-webpack-plugin';
import dotenv from 'dotenv';
import gitRevSync from 'git-rev-sync';
import HtmlWebpackPlugin from 'html-webpack-plugin';
import MiniCssExtractPlugin from 'mini-css-extract-plugin';
import { DefinePlugin, ProvidePlugin } from 'webpack';
import type { Configuration } from 'webpack';

import packageJson from '../../package.json';

const SDK_ROOT = resolve(__dirname, '..', '..', '..', '..', 'sdk');
const PROJECT_ROOT = resolve(__dirname, '..', '..');
const CONFIGS_ROOT = resolve(PROJECT_ROOT, 'configs');
const SRC_ROOT = resolve(PROJECT_ROOT, 'src');
const OUTPUT_ROOT = resolve(PROJECT_ROOT, 'dist');
const TS_CONFIGS_ROOT = resolve(CONFIGS_ROOT, 'ts');
const BUILD_ENV = process.env.BUILD_ENV as 'production' | 'development' | 'nightly' | 'rc';
const IS_NIGHTLY = BUILD_ENV === 'nightly';
const IS_DEV = BUILD_ENV === 'development';
const IS_PROD = BUILD_ENV === 'production';
const IS_RC = BUILD_ENV === 'rc';
const RC_VERSION = IS_RC ? Number(process.env.RC_VERSION) || 0 : undefined;

const TS_CONFIG_FILE = resolve(TS_CONFIGS_ROOT, `tsconfig.${IS_DEV ? 'dev' : 'prod'}.json`);
const APP_NAME = IS_RC
    ? 'IOTA Wallet (RC)'
    : IS_DEV
      ? 'IOTA Wallet (DEV)'
      : IS_NIGHTLY
        ? 'IOTA Wallet (Nightly)'
        : 'IOTA Wallet';

dotenv.config({
    path: [resolve(SDK_ROOT, '.env'), resolve(SDK_ROOT, '.env.defaults')],
});

// From package.json: x.y.z
// App version (production): x.y.z
// App version (rc): x.y.z.n
function generateVersion(n: number | undefined) {
    const sha = gitRevSync.short();
    const packageVersion = packageJson.version;
    const version = n !== undefined ? `${packageVersion}.${n}` : packageVersion;
    const version_name = n !== undefined ? `${packageVersion}-rc.${n}` : packageVersion;
    return {
        version,
        version_name: `${version_name} (${sha})`,
    };
}

function loadTsConfig(tsConfigFilePath: string) {
    return new Promise<string>((res, rej) => {
        exec(
            `${resolve(
                PROJECT_ROOT,
                'node_modules',
                '.bin',
                'tsc',
            )} -p ${tsConfigFilePath} --showConfig`,
            (error, stdout, stderr) => {
                if (error || stderr) {
                    rej(error || stderr);
                }
                res(stdout);
            },
        );
    }).then(
        (tsContent) => JSON.parse(tsContent),
        (e) => {
            // eslint-disable-next-line no-console
            console.error(e);
            throw e;
        },
    );
}

async function generateAliasFromTs() {
    const tsConfigJSON = await loadTsConfig(TS_CONFIG_FILE);
    const {
        compilerOptions: { paths, baseUrl = './' },
    } = tsConfigJSON;
    const alias: Record<string, string> = {};
    if (paths) {
        Object.keys(paths).forEach((anAlias) => {
            const aliasPath = paths[anAlias][0];
            const adjAlias = anAlias.replace(/\/\*$/gi, '');
            const adjPath = (
                aliasPath.startsWith('./') || aliasPath.startsWith('../')
                    ? resolve(TS_CONFIGS_ROOT, baseUrl, aliasPath)
                    : aliasPath
            ).replace(/\/\*$/, '');
            alias[adjAlias] = adjPath;
        });
    }
    return alias;
}

const commonConfig: () => Promise<Configuration> = async () => {
    const alias = await generateAliasFromTs();
    const walletVersionDetails = generateVersion(RC_VERSION);
    const sentryAuthToken = process.env.SENTRY_AUTH_TOKEN;
    return {
        context: SRC_ROOT,
        entry: {
            background: './background',
            ui: './ui',
            'content-script': './content-script',
            'dapp-interface': './dapp-interface',
        },
        output: {
            path: OUTPUT_ROOT,
            clean: true,
        },
        stats: {
            preset: 'summary',
            timings: true,
            errors: true,
            warnings: true,
        },
        resolve: {
            extensions: ['.ts', '.tsx', '.js'],
            // Fix .js imports from @iota/iota-sdk since we are importing it from source
            extensionAlias: {
                '.js': ['.js', '.ts', '.tsx', '.jsx'],
                '.mjs': ['.mjs', '.mts'],
                '.cjs': ['.cjs', '.cts'],
            },
            alias,
            fallback: {
                crypto: false,
                stream: require.resolve('stream-browserify'),
                buffer: require.resolve('buffer/'),
            },
        },
        module: {
            rules: [
                {
                    test: /\.(t|j)sx?$/,
                    loader: 'ts-loader',
                    options: {
                        configFile: TS_CONFIG_FILE,
                    },
                    exclude: /node_modules/,
                },
                {
                    test: /\.(s)?css$/i,
                    use: [
                        MiniCssExtractPlugin.loader,
                        {
                            loader: 'css-loader',
                            options: {
                                modules: {
                                    auto: true,
                                    localIdentName: IS_DEV
                                        ? '[name]__[local]__[hash:base64:8]'
                                        : '[hash:base64]',
                                    exportLocalsConvention: 'dashes',
                                },
                            },
                        },
                        'postcss-loader',
                        'sass-loader',
                    ],
                },
                {
                    test: /\.(png|jpg|jpeg|gif)$/,
                    type: 'asset/resource',
                },
                {
                    test: /\.svg$/i,
                    issuer: /\.[jt]sx?$/,
                    resourceQuery: { not: [/url/] },
                    use: ['@svgr/webpack'],
                },
            ],
        },
        plugins: [
            new MiniCssExtractPlugin(),
            new HtmlWebpackPlugin({
                chunks: ['ui'],
                filename: 'ui.html',
                template: resolve(SRC_ROOT, 'ui', 'index.template.html'),
                title: APP_NAME,
            }),
            new CopyPlugin({
                patterns: [
                    {
                        from: resolve(SRC_ROOT, 'manifest', 'icons', '**', '*'),
                    },
                    {
                        from: resolve(SRC_ROOT, 'manifest', 'manifest.json'),
                        to: resolve(OUTPUT_ROOT, '[name][ext]'),
                        transform: (content) => {
                            const manifestJson = {
                                ...JSON.parse(content.toString()),
                                ...walletVersionDetails,
                                name: APP_NAME,
                                description: packageJson.description,
                                ...(IS_DEV
                                    ? {
                                          key: 'MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1uOw+go7YTI8xNuHzIQC7J7x6Jw0in2UPptgcR1c+7aA3XSb03TVZkMXSMslCB7KTpaJOOQK1urLN3/FFos55f3vXixsjHT3pVbX/FhUmBK0kLOx8kl5Ns9ywVgBJyCSEnMrR1IlbiiU8GoXH1Bzb4SxkDELSQIZRetd+zTnwsUx/74grPT4EmgVglHBBYO75iJMsiJ/F0zMEsEQ+0SDfmU0v5Qh8slzryZr+8p8Q/mpNADzwS51o74feeGC5nAM5IgX0LyjRzCLXsiEmdC57KOaCpI7yhzDjXpye374oTdZJulv/tVeA1ymLIKQM5rfyeoqnxSOMsgGsvoM60WoYQIDAQAB',
                                      }
                                    : undefined),
                                icons: {
                                    16: `manifest/icons/iota-icon-16${IS_RC ? '-rc' : ''}.png`,
                                    32: `manifest/icons/iota-icon-32${IS_RC ? '-rc' : ''}.png`,
                                    48: `manifest/icons/iota-icon-48${IS_RC ? '-rc' : ''}.png`,
                                    128: `manifest/icons/iota-icon-128${IS_RC ? '-rc' : ''}.png`,
                                },
                            };
                            return JSON.stringify(manifestJson, null, 4);
                        },
                    },
                ],
            }),
            new DefinePlugin({
                // This brakes bg service, js-sha3 checks if window is defined,
                // but it's not defined in background service.
                // TODO: check if this is worth investigating a fix and maybe do a separate build for UI and bg?
                // 'typeof window': JSON.stringify(typeof {}),
                'process.env.NODE_DEBUG': false,
                'process.env.WALLET_KEYRING_PASSWORD': JSON.stringify(
                    IS_DEV ? 'DEV_PASS' : Buffer.from(randomBytes(64)).toString('hex'),
                ),
                'process.env.IS_RC': IS_RC,
                'process.env.APP_NAME': JSON.stringify(APP_NAME),
                'process.env.DEFAULT_NETWORK': JSON.stringify(process.env.DEFAULT_NETWORK),
                'process.env.IOTA_NETWORKS': JSON.stringify(process.env.IOTA_NETWORKS),
                'process.env.APPS_BACKEND': JSON.stringify(process.env.APPS_BACKEND),
                'process.env.SENTRY_AUTH_TOKEN': JSON.stringify(process.env.SENTRY_AUTH_TOKEN),
                'process.env.BUILD_ENV': JSON.stringify(BUILD_ENV),
            }),
            new ProvidePlugin({
                Buffer: ['buffer', 'Buffer'],
            }),
            new SentryWebpackPlugin({
                org: 'iota-foundation-eu',
                project: 'iota-wallet', // Sentry dev hint: use 'iota-wallet-dev' project for testing
                include: OUTPUT_ROOT,
                dryRun: !IS_PROD || !sentryAuthToken,
                authToken: sentryAuthToken,
                release: walletVersionDetails.version,
                silent: true,
            }),
        ],
    };
};

export default commonConfig;
