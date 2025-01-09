// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createMessage } from '_messages';
import { WindowMessageStream } from '_src/shared/messaging/windowMessageStream';
import type { BasePayload, Payload } from '_payloads';
import type { GetAccount } from '_src/shared/messaging/messages/payloads/account/getAccount';
import type { GetAccountResponse } from '_src/shared/messaging/messages/payloads/account/getAccountResponse';
import type { SetNetworkPayload } from '_payloads/network';
import {
    ALL_PERMISSION_TYPES,
    type AcquirePermissionsRequest,
    type AcquirePermissionsResponse,
    type HasPermissionsRequest,
    type HasPermissionsResponse,
} from '_payloads/permissions';
import type {
    ExecuteTransactionRequest,
    ExecuteTransactionResponse,
    SignTransactionRequest,
    SignTransactionResponse,
} from '_payloads/transactions';
import { getCustomNetwork, type NetworkEnvType } from '@iota/core';
import { type SignMessageRequest } from '_src/shared/messaging/messages/payloads/transactions/signMessage';
import { isWalletStatusChangePayload } from '_src/shared/messaging/messages/payloads/wallet-status-change';
import { getNetwork, Network, type ChainType } from '@iota/iota-sdk/client';
import { fromB64, toB64 } from '@iota/iota-sdk/utils';
import {
    ReadonlyWalletAccount,
    SUPPORTED_CHAINS,
    type StandardConnectFeature,
    type StandardConnectMethod,
    type StandardEventsFeature,
    type StandardEventsListeners,
    type StandardEventsOnMethod,
    type IotaFeatures,
    type IotaSignPersonalMessageMethod,
    type Wallet,
    type IotaSignTransactionMethod,
    type IotaSignAndExecuteTransactionMethod,
} from '@iota/wallet-standard';
import mitt, { type Emitter } from 'mitt';
import { filter, map, type Observable } from 'rxjs';

import { mapToPromise } from './utils';
import { bcs } from '@iota/iota-sdk/bcs';
import { generateWalletMessageStreamIdentifiers } from '_src/shared/utils/generateWalletMessageStreamIdentifiers';
import { DEFAULT_APP_NAME } from '_src/shared/constants';

type WalletEventsMap = {
    [E in keyof StandardEventsListeners]: Parameters<StandardEventsListeners[E]>[0];
};

// NOTE: Because this runs in a content script, we can't fetch the manifest.
const NAME = process.env.APP_NAME || DEFAULT_APP_NAME;

export class IotaWallet implements Wallet {
    readonly #events: Emitter<WalletEventsMap>;
    readonly #version = '1.0.0' as const;
    readonly #name = NAME;
    #accounts: ReadonlyWalletAccount[];
    #messagesStream: WindowMessageStream;
    #activeChain: ChainType | null = null;

    get version() {
        return this.#version;
    }

    get name() {
        return this.#name;
    }

    get icon() {
        return 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjU2IiBoZWlnaHQ9IjI1NiIgdmlld0JveD0iMCAwIDI1NiAyNTYiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+CjxyZWN0IHdpZHRoPSIyNTYiIGhlaWdodD0iMjU2IiByeD0iMTI4IiBmaWxsPSJ3aGl0ZSIvPgo8cGF0aCBkPSJNMTY5LjIyNyA2MS42Mjk1QzE2OS4yMjcgNjYuMzk1NCAxNjUuMzc1IDcwLjI1OSAxNjAuNjI0IDcwLjI1OUMxNTUuODczIDcwLjI1OSAxNTIuMDIxIDY2LjM5NTQgMTUyLjAyMSA2MS42Mjk1QzE1Mi4wMjEgNTYuODYzNSAxNTUuODczIDUzIDE2MC42MjQgNTNDMTY1LjM3NSA1MyAxNjkuMjI3IDU2Ljg2MzUgMTY5LjIyNyA2MS42Mjk1WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTc4Ljg5OSAxODMuNTMxQzE3OC44OTkgMTg4LjI5NiAxNzUuMDQ3IDE5Mi4xNiAxNzAuMjk2IDE5Mi4xNkMxNjUuNTQ0IDE5Mi4xNiAxNjEuNjkzIDE4OC4yOTYgMTYxLjY5MyAxODMuNTMxQzE2MS42OTMgMTc4Ljc2NSAxNjUuNTQ0IDE3NC45MDEgMTcwLjI5NiAxNzQuOTAxQzE3NS4wNDcgMTc0LjkwMSAxNzguODk5IDE3OC43NjUgMTc4Ljg5OSAxODMuNTMxWiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTYzLjA4MyA5MS44MzFDMTY3LjA3IDkxLjgzMSAxNzAuMzAyIDg4LjU4OTEgMTcwLjMwMiA4NC41OUMxNzAuMzAyIDgwLjU5MDkgMTY3LjA3IDc3LjM0OSAxNjMuMDgzIDc3LjM0OUMxNTkuMDk3IDc3LjM0OSAxNTUuODY1IDgwLjU5MDkgMTU1Ljg2NSA4NC41OUMxNTUuODY1IDg4LjU4OTEgMTU5LjA5NyA5MS44MzEgMTYzLjA4MyA5MS44MzFaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xODkuNSA4Ny4zNjZDMTg5LjUgOTEuMzY1MSAxODYuMjY4IDk0LjYwNyAxODIuMjgxIDk0LjYwN0MxNzguMjk1IDk0LjYwNyAxNzUuMDYyIDkxLjM2NTEgMTc1LjA2MiA4Ny4zNjZDMTc1LjA2MiA4My4zNjY5IDE3OC4yOTUgODAuMTI1IDE4Mi4yODEgODAuMTI1QzE4Ni4yNjggODAuMTI1IDE4OS41IDgzLjM2NjkgMTg5LjUgODcuMzY2WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTYwLjYyNiAxMTAuMTg2QzE2NC4wMjEgMTEwLjE4NiAxNjYuNzc0IDEwNy40MjYgMTY2Ljc3NCAxMDQuMDJDMTY2Ljc3NCAxMDAuNjE1IDE2NC4wMjEgOTcuODU0MiAxNjAuNjI2IDk3Ljg1NDJDMTU3LjIzMSA5Ny44NTQyIDE1NC40NzkgMTAwLjYxNSAxNTQuNDc5IDEwNC4wMkMxNTQuNDc5IDEwNy40MjYgMTU3LjIzMSAxMTAuMTg2IDE2MC42MjYgMTEwLjE4NloiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE4NS45NzcgMTA2LjYyNEMxODUuOTc3IDExMC4wMyAxODMuMjI1IDExMi43OSAxNzkuODMgMTEyLjc5QzE3Ni40MzQgMTEyLjc5IDE3My42ODIgMTEwLjAzIDE3My42ODIgMTA2LjYyNEMxNzMuNjgyIDEwMy4yMTkgMTc2LjQzNCAxMDAuNDU4IDE3OS44MyAxMDAuNDU4QzE4My4yMjUgMTAwLjQ1OCAxODUuOTc3IDEwMy4yMTkgMTg1Ljk3NyAxMDYuNjI0WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTk1LjE4OSAxMTguODA2QzE5OC41ODQgMTE4LjgwNiAyMDEuMzM2IDExNi4wNDUgMjAxLjMzNiAxMTIuNjRDMjAxLjMzNiAxMDkuMjM1IDE5OC41ODQgMTA2LjQ3NCAxOTUuMTg5IDEwNi40NzRDMTkxLjc5NCAxMDYuNDc0IDE4OS4wNDIgMTA5LjIzNSAxODkuMDQyIDExMi42NEMxODkuMDQyIDExNi4wNDUgMTkxLjc5NCAxMTguODA2IDE5NS4xODkgMTE4LjgwNloiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE3OS41MTIgMTIyLjE5N0MxNzkuNTEyIDEyNS4xNzQgMTc3LjEwNiAxMjcuNTg3IDE3NC4xMzkgMTI3LjU4N0MxNzEuMTcxIDEyNy41ODcgMTY4Ljc2NiAxMjUuMTc0IDE2OC43NjYgMTIyLjE5N0MxNjguNzY2IDExOS4yMiAxNzEuMTcxIDExNi44MDcgMTc0LjEzOSAxMTYuODA3QzE3Ny4xMDYgMTE2LjgwNyAxNzkuNTEyIDExOS4yMiAxNzkuNTEyIDEyMi4xOTdaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xNTQuOTQxIDEyNC45NjJDMTU3LjkwOSAxMjQuOTYyIDE2MC4zMTQgMTIyLjU0OSAxNjAuMzE0IDExOS41NzJDMTYwLjMxNCAxMTYuNTk1IDE1Ny45MDkgMTE0LjE4MiAxNTQuOTQxIDExNC4xODJDMTUxLjk3MyAxMTQuMTgyIDE0OS41NjggMTE2LjU5NSAxNDkuNTY4IDExOS41NzJDMTQ5LjU2OCAxMjIuNTQ5IDE1MS45NzMgMTI0Ljk2MiAxNTQuOTQxIDEyNC45NjJaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xOTUuMDM4IDEyOC4xOTdDMTk1LjAzOCAxMzEuMTc0IDE5Mi42MzIgMTMzLjU4NyAxODkuNjY1IDEzMy41ODdDMTg2LjY5NyAxMzMuNTg3IDE4NC4yOTIgMTMxLjE3NCAxODQuMjkyIDEyOC4xOTdDMTg0LjI5MiAxMjUuMjIgMTg2LjY5NyAxMjIuODA3IDE4OS42NjUgMTIyLjgwN0MxOTIuNjMyIDEyMi44MDcgMTk1LjAzOCAxMjUuMjIgMTk1LjAzOCAxMjguMTk3WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTY2LjQ1OCAxMzguODQ1QzE2OS4wMDYgMTM4Ljg0NSAxNzEuMDcyIDEzNi43NzMgMTcxLjA3MiAxMzQuMjE3QzE3MS4wNzIgMTMxLjY2MSAxNjkuMDA2IDEyOS41ODkgMTY2LjQ1OCAxMjkuNTg5QzE2My45MSAxMjkuNTg5IDE2MS44NDQgMTMxLjY2MSAxNjEuODQ0IDEzNC4yMTdDMTYxLjg0NCAxMzYuNzczIDE2My45MSAxMzguODQ1IDE2Ni40NTggMTM4Ljg0NVoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE4Ni42MDMgMTQwLjA2NkMxODYuNjAzIDE0Mi42MjIgMTg0LjUzNyAxNDQuNjk0IDE4MS45ODkgMTQ0LjY5NEMxNzkuNDQxIDE0NC42OTQgMTc3LjM3NSAxNDIuNjIyIDE3Ny4zNzUgMTQwLjA2NkMxNzcuMzc1IDEzNy41MSAxNzkuNDQxIDEzNS40MzggMTgxLjk4OSAxMzUuNDM4QzE4NC41MzcgMTM1LjQzOCAxODYuNjAzIDEzNy41MSAxODYuNjAzIDE0MC4wNjZaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xNDcuNDExIDEzNi4wNzRDMTQ5Ljk1OSAxMzYuMDc0IDE1Mi4wMjUgMTM0LjAwMiAxNTIuMDI1IDEzMS40NDZDMTUyLjAyNSAxMjguODkgMTQ5Ljk1OSAxMjYuODE4IDE0Ny40MTEgMTI2LjgxOEMxNDQuODYzIDEyNi44MTggMTQyLjc5NyAxMjguODkgMTQyLjc5NyAxMzEuNDQ2QzE0Mi43OTcgMTM0LjAwMiAxNDQuODYzIDEzNi4wNzQgMTQ3LjQxMSAxMzYuMDc0WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTYxLjg0NyAxNDIuNTM0QzE2MS44NDcgMTQ0LjY2MiAxNjAuMTI4IDE0Ni4zODYgMTU4LjAwNyAxNDYuMzg2QzE1NS44ODYgMTQ2LjM4NiAxNTQuMTY3IDE0NC42NjIgMTU0LjE2NyAxNDIuNTM0QzE1NC4xNjcgMTQwLjQwNyAxNTUuODg2IDEzOC42ODIgMTU4LjAwNyAxMzguNjgyQzE2MC4xMjggMTM4LjY4MiAxNjEuODQ3IDE0MC40MDcgMTYxLjg0NyAxNDIuNTM0WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTY0LjYxMyAxNTcuMDI4QzE2Ni40NzkgMTU3LjAyOCAxNjcuOTkyIDE1NS41MTEgMTY3Ljk5MiAxNTMuNjM5QzE2Ny45OTIgMTUxLjc2NyAxNjYuNDc5IDE1MC4yNSAxNjQuNjEzIDE1MC4yNUMxNjIuNzQ3IDE1MC4yNSAxNjEuMjM0IDE1MS43NjcgMTYxLjIzNCAxNTMuNjM5QzE2MS4yMzQgMTU1LjUxMSAxNjIuNzQ3IDE1Ny4wMjggMTY0LjYxMyAxNTcuMDI4WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTU5LjI0IDE1Ni40MTJDMTU5LjI0IDE1Ny45NDYgMTU4LjAwMSAxNTkuMTg5IDE1Ni40NzIgMTU5LjE4OUMxNTQuOTQzIDE1OS4xODkgMTUzLjcwMyAxNTcuOTQ2IDE1My43MDMgMTU2LjQxMkMxNTMuNzAzIDE1NC44NzkgMTU0Ljk0MyAxNTMuNjM1IDE1Ni40NzIgMTUzLjYzNUMxNTguMDAxIDE1My42MzUgMTU5LjI0IDE1NC44NzkgMTU5LjI0IDE1Ni40MTJaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xNDkuMjU0IDE1MS4xNjlDMTUxLjEyIDE1MS4xNjkgMTUyLjYzMiAxNDkuNjUxIDE1Mi42MzIgMTQ3Ljc4QzE1Mi42MzIgMTQ1LjkwOCAxNTEuMTIgMTQ0LjM5MSAxNDkuMjU0IDE0NC4zOTFDMTQ3LjM4OCAxNDQuMzkxIDE0NS44NzUgMTQ1LjkwOCAxNDUuODc1IDE0Ny43OEMxNDUuODc1IDE0OS42NTEgMTQ3LjM4OCAxNTEuMTY5IDE0OS4yNTQgMTUxLjE2OVoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE0Mi42NDQgMTM5LjkyNUMxNDIuNjQ0IDE0Mi4wNTIgMTQwLjkyNSAxNDMuNzc3IDEzOC44MDQgMTQzLjc3N0MxMzYuNjgzIDE0My43NzcgMTM0Ljk2NCAxNDIuMDUyIDEzNC45NjQgMTM5LjkyNUMxMzQuOTY0IDEzNy43OTcgMTM2LjY4MyAxMzYuMDczIDEzOC44MDQgMTM2LjA3M0MxNDAuOTI1IDEzNi4wNzMgMTQyLjY0NCAxMzcuNzk3IDE0Mi42NDQgMTM5LjkyNVoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTEzOC42NTggMTE0Ljc5OEMxNDAuNzc5IDExNC43OTggMTQyLjQ5OCAxMTMuMDczIDE0Mi40OTggMTEwLjk0NkMxNDIuNDk4IDEwOC44MTggMTQwLjc3OSAxMDcuMDk0IDEzOC42NTggMTA3LjA5NEMxMzYuNTM3IDEwNy4wOTQgMTM0LjgxOCAxMDguODE4IDEzNC44MTggMTEwLjk0NkMxMzQuODE4IDExMy4wNzMgMTM2LjUzNyAxMTQuNzk4IDEzOC42NTggMTE0Ljc5OFoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE0My43MTYgOTcuODQ3NEMxNDMuNzE2IDk5LjcxOTIgMTQyLjIwMyAxMDEuMjM3IDE0MC4zMzcgMTAxLjIzN0MxMzguNDcxIDEwMS4yMzcgMTM2Ljk1OCA5OS43MTkyIDEzNi45NTggOTcuODQ3NEMxMzYuOTU4IDk1Ljk3NTcgMTM4LjQ3MSA5NC40NTgzIDE0MC4zMzcgOTQuNDU4M0MxNDIuMjAzIDk0LjQ1ODMgMTQzLjcxNiA5NS45NzU3IDE0My43MTYgOTcuODQ3NFoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE0NC4wMjkgOTAuMTQ3N0MxNDUuNTU4IDkwLjE0NzcgMTQ2Ljc5NyA4OC45MDQ0IDE0Ni43OTcgODcuMzcwN0MxNDYuNzk3IDg1LjgzNyAxNDUuNTU4IDg0LjU5MzggMTQ0LjAyOSA4NC41OTM4QzE0Mi41IDg0LjU5MzggMTQxLjI2IDg1LjgzNyAxNDEuMjYgODcuMzcwN0MxNDEuMjYgODguOTA0NCAxNDIuNSA5MC4xNDc3IDE0NC4wMjkgOTAuMTQ3N1oiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE0MC45NjEgODEuNTE0MUMxNDAuOTYxIDgzLjM4NTggMTM5LjQ0OCA4NC45MDMyIDEzNy41ODIgODQuOTAzMkMxMzUuNzE2IDg0LjkwMzIgMTM0LjIwMyA4My4zODU4IDEzNC4yMDMgODEuNTE0MUMxMzQuMjAzIDc5LjY0MjMgMTM1LjcxNiA3OC4xMjUgMTM3LjU4MiA3OC4xMjVDMTM5LjQ0OCA3OC4xMjUgMTQwLjk2MSA3OS42NDIzIDE0MC45NjEgODEuNTE0MVoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTE3My4zODIgMTUyLjI1NkMxNzUuNTAzIDE1Mi4yNTYgMTc3LjIyMiAxNTAuNTMxIDE3Ny4yMjIgMTQ4LjQwNEMxNzcuMjIyIDE0Ni4yNzcgMTc1LjUwMyAxNDQuNTUyIDE3My4zODIgMTQ0LjU1MkMxNzEuMjYxIDE0NC41NTIgMTY5LjU0MiAxNDYuMjc3IDE2OS41NDIgMTQ4LjQwNEMxNjkuNTQyIDE1MC41MzEgMTcxLjI2MSAxNTIuMjU2IDE3My4zODIgMTUyLjI1NloiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTEzMi42NTkgNzYuNTcwN0MxMzIuNjU5IDc4LjY5OCAxMzAuOTQgODAuNDIyNiAxMjguODE5IDgwLjQyMjZDMTI2LjY5OCA4MC40MjI2IDEyNC45NzkgNzguNjk4IDEyNC45NzkgNzYuNTcwN0MxMjQuOTc5IDc0LjQ0MzMgMTI2LjY5OCA3Mi43MTg4IDEyOC44MTkgNzIuNzE4OEMxMzAuOTQgNzIuNzE4OCAxMzIuNjU5IDc0LjQ0MzMgMTMyLjY1OSA3Ni41NzA3WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTMxLjQzOSA5Ni43NjExQzEzMy41NiA5Ni43NjExIDEzNS4yNzkgOTUuMDM2NiAxMzUuMjc5IDkyLjkwOTJDMTM1LjI3OSA5MC43ODE5IDEzMy41NiA4OS4wNTczIDEzMS40MzkgODkuMDU3M0MxMjkuMzE4IDg5LjA1NzMgMTI3LjU5OSA5MC43ODE5IDEyNy41OTkgOTIuOTA5MkMxMjcuNTk5IDk1LjAzNjYgMTI5LjMxOCA5Ni43NjExIDEzMS40MzkgOTYuNzYxMVoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTEzMS43NDkgMTA3LjcwNkMxMzEuNzQ5IDExMC4yNjMgMTI5LjY4MyAxMTIuMzM1IDEyNy4xMzUgMTEyLjMzNUMxMjQuNTg3IDExMi4zMzUgMTIyLjUyMSAxMTAuMjYzIDEyMi41MjEgMTA3LjcwNkMxMjIuNTIxIDEwNS4xNSAxMjQuNTg3IDEwMy4wNzggMTI3LjEzNSAxMDMuMDc4QzEyOS42ODMgMTAzLjA3OCAxMzEuNzQ5IDEwNS4xNSAxMzEuNzQ5IDEwNy43MDZaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMTMuMTQ0IDExMi4zMzdDMTE2LjExMiAxMTIuMzM3IDExOC41MTcgMTA5LjkyNCAxMTguNTE3IDEwNi45NDdDMTE4LjUxNyAxMDMuOTcgMTE2LjExMiAxMDEuNTU3IDExMy4xNDQgMTAxLjU1N0MxMTAuMTc3IDEwMS41NTcgMTA3Ljc3MSAxMDMuOTcgMTA3Ljc3MSAxMDYuOTQ3QzEwNy43NzEgMTA5LjkyNCAxMTAuMTc3IDExMi4zMzcgMTEzLjE0NCAxMTIuMzM3WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTAzLjAwOCAxMDkuODY5QzEwMy4wMDggMTEzLjI3NSAxMDAuMjU2IDExNi4wMzUgOTYuODYwOCAxMTYuMDM1QzkzLjQ2NTcgMTE2LjAzNSA5MC43MTM1IDExMy4yNzUgOTAuNzEzNSAxMDkuODY5QzkwLjcxMzUgMTA2LjQ2NCA5My40NjU3IDEwMy43MDMgOTYuODYwOCAxMDMuNzAzQzEwMC4yNTYgMTAzLjcwMyAxMDMuMDA4IDEwNi40NjQgMTAzLjAwOCAxMDkuODY5WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNNzkuMDQ3IDEyNC44MTVDODMuMDMzOSAxMjQuODE1IDg2LjI2NTkgMTIxLjU3MyA4Ni4yNjU5IDExNy41NzRDODYuMjY1OSAxMTMuNTc1IDgzLjAzMzkgMTEwLjMzMyA3OS4wNDcgMTEwLjMzM0M3NS4wNjAxIDExMC4zMzMgNzEuODI4MSAxMTMuNTc1IDcxLjgyODEgMTE3LjU3NEM3MS44MjgxIDEyMS41NzMgNzUuMDYwMSAxMjQuODE1IDc5LjA0NyAxMjQuODE1WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNNjkuMjA2MyAxMzAuOTg0QzY5LjIwNjMgMTM1Ljc1IDY1LjM1NDUgMTM5LjYxMyA2MC42MDMxIDEzOS42MTNDNTUuODUxOCAxMzkuNjEzIDUyIDEzNS43NSA1MiAxMzAuOTg0QzUyIDEyNi4yMTggNTUuODUxOCAxMjIuMzU0IDYwLjYwMzEgMTIyLjM1NEM2NS4zNTQ1IDEyMi4zNTQgNjkuMjA2MyAxMjYuMjE4IDY5LjIwNjMgMTMwLjk4NFoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTcxLjgyODMgMTA2Ljc3OUM3NS44MTUyIDEwNi43NzkgNzkuMDQ3MiAxMDMuNTM3IDc5LjA0NzIgOTkuNTM3OUM3OS4wNDcyIDk1LjUzODggNzUuODE1MiA5Mi4yOTY5IDcxLjgyODMgOTIuMjk2OUM2Ny44NDE0IDkyLjI5NjkgNjQuNjA5NCA5NS41Mzg4IDY0LjYwOTQgOTkuNTM3OUM2NC42MDk0IDEwMy41MzcgNjcuODQxNCAxMDYuNzc5IDcxLjgyODMgMTA2Ljc3OVoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTk1Ljc4OTIgOTEuODMyN0M5NS43ODkyIDk1LjIzODEgOTMuMDM3IDk3Ljk5ODcgODkuNjQyIDk3Ljk5ODdDODYuMjQ3IDk3Ljk5ODcgODMuNDk0OCA5NS4yMzgxIDgzLjQ5NDggOTEuODMyN0M4My40OTQ4IDg4LjQyNzMgODYuMjQ3IDg1LjY2NjcgODkuNjQyIDg1LjY2NjdDOTMuMDM3IDg1LjY2NjcgOTUuNzg5MiA4OC40MjczIDk1Ljc4OTIgOTEuODMyN1oiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTg3LjAzNzkgODEuNjY1NEM5MC40MzI5IDgxLjY2NTQgOTMuMTg1MSA3OC45MDQ4IDkzLjE4NTEgNzUuNDk5NEM5My4xODUxIDcyLjA5NCA5MC40MzI5IDY5LjMzMzMgODcuMDM3OSA2OS4zMzMzQzgzLjY0MjggNjkuMzMzMyA4MC44OTA2IDcyLjA5NCA4MC44OTA2IDc1LjQ5OTRDODAuODkwNiA3OC45MDQ4IDgzLjY0MjggODEuNjY1NCA4Ny4wMzc5IDgxLjY2NTRaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMDguNjk0IDcyLjU3NzJDMTA4LjY5NCA3NS41NTM4IDEwNi4yODkgNzcuOTY2OSAxMDMuMzIxIDc3Ljk2NjlDMTAwLjM1NCA3Ny45NjY5IDk3Ljk0NzkgNzUuNTUzOCA5Ny45NDc5IDcyLjU3NzJDOTcuOTQ3OSA2OS42MDA1IDEwMC4zNTQgNjcuMTg3NSAxMDMuMzIxIDY3LjE4NzVDMTA2LjI4OSA2Ny4xODc1IDEwOC42OTQgNjkuNjAwNSAxMDguNjk0IDcyLjU3NzJaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMTcuMjk2IDc3Ljk3NTNDMTE5Ljg0NSA3Ny45NzUzIDEyMS45MTEgNzUuOTAzMSAxMjEuOTExIDczLjM0N0MxMjEuOTExIDcwLjc5MDkgMTE5Ljg0NSA2OC43MTg4IDExNy4yOTYgNjguNzE4OEMxMTQuNzQ4IDY4LjcxODggMTEyLjY4MiA3MC43OTA5IDExMi42ODIgNzMuMzQ3QzExMi42ODIgNzUuOTAzMSAxMTQuNzQ4IDc3Ljk3NTMgMTE3LjI5NiA3Ny45NzUzWiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTI0LjUxNSA4OS42ODU2QzEyNC41MTUgOTIuMjQxNyAxMjIuNDQ5IDk0LjMxMzggMTE5LjkwMSA5NC4zMTM4QzExNy4zNTIgOTQuMzEzOCAxMTUuMjg2IDkyLjI0MTcgMTE1LjI4NiA4OS42ODU2QzExNS4yODYgODcuMTI5NCAxMTcuMzUyIDg1LjA1NzMgMTE5LjkwMSA4NS4wNTczQzEyMi40NDkgODUuMDU3MyAxMjQuNTE1IDg3LjEyOTQgMTI0LjUxNSA4OS42ODU2WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTA1LjkyNSA5NC40NTEzQzEwOC44OTMgOTQuNDUxMyAxMTEuMjk5IDkyLjAzODIgMTExLjI5OSA4OS4wNjE2QzExMS4yOTkgODYuMDg0OSAxMDguODkzIDgzLjY3MTkgMTA1LjkyNSA4My42NzE5QzEwMi45NTggODMuNjcxOSAxMDAuNTUyIDg2LjA4NDkgMTAwLjU1MiA4OS4wNjE2QzEwMC41NTIgOTIuMDM4MiAxMDIuOTU4IDk0LjQ1MTMgMTA1LjkyNSA5NC40NTEzWiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNOTMuNDg0OSAxMzIuNjgzQzkzLjQ4NDkgMTM0LjIxNyA5Mi4yNDU0IDEzNS40NiA5MC43MTY0IDEzNS40NkM4OS4xODc0IDEzNS40NiA4Ny45NDc5IDEzNC4yMTcgODcuOTQ3OSAxMzIuNjgzQzg3Ljk0NzkgMTMxLjE1IDg5LjE4NzQgMTI5LjkwNiA5MC43MTY0IDEyOS45MDZDOTIuMjQ1NCAxMjkuOTA2IDkzLjQ4NDkgMTMxLjE1IDkzLjQ4NDkgMTMyLjY4M1oiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTEwMS43NzUgMTM0LjA1OUMxMDMuNjQxIDEzNC4wNTkgMTA1LjE1MyAxMzIuNTQyIDEwNS4xNTMgMTMwLjY3QzEwNS4xNTMgMTI4Ljc5OSAxMDMuNjQxIDEyNy4yODEgMTAxLjc3NSAxMjcuMjgxQzk5LjkwODUgMTI3LjI4MSA5OC4zOTU4IDEyOC43OTkgOTguMzk1OCAxMzAuNjdDOTguMzk1OCAxMzIuNTQyIDk5LjkwODUgMTM0LjA1OSAxMDEuNzc1IDEzNC4wNTlaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMTcuNzU4IDEyNS41ODZDMTE3Ljc1OCAxMjcuNzE0IDExNi4wMzkgMTI5LjQzOCAxMTMuOTE4IDEyOS40MzhDMTExLjc5NyAxMjkuNDM4IDExMC4wNzggMTI3LjcxNCAxMTAuMDc4IDEyNS41ODZDMTEwLjA3OCAxMjMuNDU5IDExMS43OTcgMTIxLjczNCAxMTMuOTE4IDEyMS43MzRDMTE2LjAzOSAxMjEuNzM0IDExNy43NTggMTIzLjQ1OSAxMTcuNzU4IDEyNS41ODZaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMTYuODM4IDE0MS43NzJDMTE5LjM4NiAxNDEuNzcyIDEyMS40NTIgMTM5LjcgMTIxLjQ1MiAxMzcuMTQ0QzEyMS40NTIgMTM0LjU4OCAxMTkuMzg2IDEzMi41MTYgMTE2LjgzOCAxMzIuNTE2QzExNC4yOSAxMzIuNTE2IDExMi4yMjQgMTM0LjU4OCAxMTIuMjI0IDEzNy4xNDRDMTEyLjIyNCAxMzkuNyAxMTQuMjkgMTQxLjc3MiAxMTYuODM4IDE0MS43NzJaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMDUuOTI1IDE0MC44NDdDMTA1LjkyNSAxNDIuOTc0IDEwNC4yMDYgMTQ0LjY5OSAxMDIuMDg1IDE0NC42OTlDOTkuOTY0MSAxNDQuNjk5IDk4LjI0NDggMTQyLjk3NCA5OC4yNDQ4IDE0MC44NDdDOTguMjQ0OCAxMzguNzE5IDk5Ljk2NDEgMTM2Ljk5NSAxMDIuMDg1IDEzNi45OTVDMTA0LjIwNiAxMzYuOTk1IDEwNS45MjUgMTM4LjcxOSAxMDUuOTI1IDE0MC44NDdaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik04OS4wMzUgMTQ0LjUzM0M5MC45MDEgMTQ0LjUzMyA5Mi40MTM3IDE0My4wMTYgOTIuNDEzNyAxNDEuMTQ0QzkyLjQxMzcgMTM5LjI3MyA5MC45MDEgMTM3Ljc1NSA4OS4wMzUgMTM3Ljc1NUM4Ny4xNjkgMTM3Ljc1NSA4NS42NTYyIDEzOS4yNzMgODUuNjU2MiAxNDEuMTQ0Qzg1LjY1NjIgMTQzLjAxNiA4Ny4xNjkgMTQ0LjUzMyA4OS4wMzUgMTQ0LjUzM1oiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTkzLjAyNDEgMTUxLjMyNkM5My4wMjQxIDE1My40NTMgOTEuMzA0OCAxNTUuMTc4IDg5LjE4MzkgMTU1LjE3OEM4Ny4wNjMgMTU1LjE3OCA4NS4zNDM4IDE1My40NTMgODUuMzQzOCAxNTEuMzI2Qzg1LjM0MzggMTQ5LjE5OSA4Ny4wNjMgMTQ3LjQ3NCA4OS4xODM5IDE0Ny40NzRDOTEuMzA0OCAxNDcuNDc0IDkzLjAyNDEgMTQ5LjE5OSA5My4wMjQxIDE1MS4zMjZaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMDQuODU0IDE1Ny4xODRDMTA3LjQwMiAxNTcuMTg0IDEwOS40NjggMTU1LjExMSAxMDkuNDY4IDE1Mi41NTVDMTA5LjQ2OCAxNDkuOTk5IDEwNy40MDIgMTQ3LjkyNyAxMDQuODU0IDE0Ny45MjdDMTAyLjMwNSAxNDcuOTI3IDEwMC4yNCAxNDkuOTk5IDEwMC4yNCAxNTIuNTU1QzEwMC4yNCAxNTUuMTExIDEwMi4zMDUgMTU3LjE4NCAxMDQuODU0IDE1Ny4xODRaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMjguNTA3IDE0OS43OEMxMjguNTA3IDE1Mi43NTcgMTI2LjEwMSAxNTUuMTcgMTIzLjEzNCAxNTUuMTdDMTIwLjE2NiAxNTUuMTcgMTE3Ljc2IDE1Mi43NTcgMTE3Ljc2IDE0OS43OEMxMTcuNzYgMTQ2LjgwNCAxMjAuMTY2IDE0NC4zOTEgMTIzLjEzNCAxNDQuMzkxQzEyNi4xMDEgMTQ0LjM5MSAxMjguNTA3IDE0Ni44MDQgMTI4LjUwNyAxNDkuNzhaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMzMuODkyIDE2OC41ODdDMTM3LjI4NyAxNjguNTg3IDE0MC4wMzkgMTY1LjgyNyAxNDAuMDM5IDE2Mi40MjFDMTQwLjAzOSAxNTkuMDE2IDEzNy4yODcgMTU2LjI1NSAxMzMuODkyIDE1Ni4yNTVDMTMwLjQ5NyAxNTYuMjU1IDEyNy43NDUgMTU5LjAxNiAxMjcuNzQ1IDE2Mi40MjFDMTI3Ljc0NSAxNjUuODI3IDEzMC40OTcgMTY4LjU4NyAxMzMuODkyIDE2OC41ODdaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMTYuNjc0IDE2NS4wM0MxMTYuNjc0IDE2OC4wMDcgMTE0LjI2OCAxNzAuNDIgMTExLjMgMTcwLjQyQzEwOC4zMzMgMTcwLjQyIDEwNS45MjcgMTY4LjAwNyAxMDUuOTI3IDE2NS4wM0MxMDUuOTI3IDE2Mi4wNTQgMTA4LjMzMyAxNTkuNjQxIDExMS4zIDE1OS42NDFDMTE0LjI2OCAxNTkuNjQxIDExNi42NzQgMTYyLjA1NCAxMTYuNjc0IDE2NS4wM1oiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTk4LjU1NTUgMTgwLjkxQzEwMS41MjMgMTgwLjkxIDEwMy45MjkgMTc4LjQ5NyAxMDMuOTI5IDE3NS41MkMxMDMuOTI5IDE3Mi41NDMgMTAxLjUyMyAxNzAuMTMgOTguNTU1NSAxNzAuMTNDOTUuNTg4IDE3MC4xMyA5My4xODIzIDE3Mi41NDMgOTMuMTgyMyAxNzUuNTJDOTMuMTgyMyAxNzguNDk3IDk1LjU4OCAxODAuOTEgOTguNTU1NSAxODAuOTFaIiBmaWxsPSIjMTcxRDI2Ii8+CjxwYXRoIGQ9Ik0xMTUuMyAxODguMzA3QzExNS4zIDE5MS43MTIgMTEyLjU0NyAxOTQuNDczIDEwOS4xNTIgMTk0LjQ3M0MxMDUuNzU3IDE5NC40NzMgMTAzLjAwNSAxOTEuNzEyIDEwMy4wMDUgMTg4LjMwN0MxMDMuMDA1IDE4NC45MDEgMTA1Ljc1NyAxODIuMTQxIDEwOS4xNTIgMTgyLjE0MUMxMTIuNTQ3IDE4Mi4xNDEgMTE1LjMgMTg0LjkwMSAxMTUuMyAxODguMzA3WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTM3LjQyMiAxOTYuOTRDMTQxLjQwOSAxOTYuOTQgMTQ0LjY0MSAxOTMuNjk4IDE0NC42NDEgMTg5LjY5OUMxNDQuNjQxIDE4NS43IDE0MS40MDkgMTgyLjQ1OCAxMzcuNDIyIDE4Mi40NThDMTMzLjQzNSAxODIuNDU4IDEzMC4yMDMgMTg1LjcgMTMwLjIwMyAxODkuNjk5QzEzMC4yMDMgMTkzLjY5OCAxMzMuNDM1IDE5Ni45NCAxMzcuNDIyIDE5Ni45NFoiIGZpbGw9IiMxNzFEMjYiLz4KPHBhdGggZD0iTTEyOC4wNiAxNzcuODMzQzEyOC4wNiAxODEuMjM4IDEyNS4zMDggMTgzLjk5OSAxMjEuOTEzIDE4My45OTlDMTE4LjUxOCAxODMuOTk5IDExNS43NjYgMTgxLjIzOCAxMTUuNzY2IDE3Ny44MzNDMTE1Ljc2NiAxNzQuNDI3IDExOC41MTggMTcxLjY2NyAxMjEuOTEzIDE3MS42NjdDMTI1LjMwOCAxNzEuNjY3IDEyOC4wNiAxNzQuNDI3IDEyOC4wNiAxNzcuODMzWiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNMTQ5LjI1NSAxODEuNTI5QzE1My4yNDIgMTgxLjUyOSAxNTYuNDc0IDE3OC4yODcgMTU2LjQ3NCAxNzQuMjg4QzE1Ni40NzQgMTcwLjI4OSAxNTMuMjQyIDE2Ny4wNDcgMTQ5LjI1NSAxNjcuMDQ3QzE0NS4yNjggMTY3LjA0NyAxNDIuMDM2IDE3MC4yODkgMTQyLjAzNiAxNzQuMjg4QzE0Mi4wMzYgMTc4LjI4NyAxNDUuMjY4IDE4MS41MjkgMTQ5LjI1NSAxODEuNTI5WiIgZmlsbD0iIzE3MUQyNiIvPgo8cGF0aCBkPSJNOTYuNzEyNyAxNjMuMDM1Qzk2LjcxMjcgMTY1LjU5MSA5NC42NDY4IDE2Ny42NjMgOTIuMDk4NSAxNjcuNjYzQzg5LjU1MDIgMTY3LjY2MyA4Ny40ODQ0IDE2NS41OTEgODcuNDg0NCAxNjMuMDM1Qzg3LjQ4NDQgMTYwLjQ3OCA4OS41NTAyIDE1OC40MDYgOTIuMDk4NSAxNTguNDA2Qzk0LjY0NjggMTU4LjQwNiA5Ni43MTI3IDE2MC40NzggOTYuNzEyNyAxNjMuMDM1WiIgZmlsbD0iIzE3MUQyNiIvPgo8L3N2Zz4K' as const;
    }

    get chains() {
        // TODO: Extract chain from wallet:
        return SUPPORTED_CHAINS;
    }

    get features(): StandardConnectFeature & StandardEventsFeature & IotaFeatures {
        return {
            'standard:connect': {
                version: '1.0.0',
                connect: this.#connect,
            },
            'standard:events': {
                version: '1.0.0',
                on: this.#on,
            },
            'iota:signTransaction': {
                version: '2.0.0',
                signTransaction: this.#signTransaction,
            },
            'iota:signAndExecuteTransaction': {
                version: '2.0.0',
                signAndExecuteTransaction: this.#signAndExecuteTransaction,
            },
            'iota:signPersonalMessage': {
                version: '1.0.0',
                signPersonalMessage: this.#signPersonalMessage,
            },
        };
    }

    get accounts() {
        return this.#accounts;
    }

    #setAccounts(accounts: GetAccountResponse['accounts']) {
        this.#accounts = accounts.map(
            ({ address, publicKey, nickname }) =>
                new ReadonlyWalletAccount({
                    address,
                    label: nickname || undefined,
                    publicKey: publicKey ? fromB64(publicKey) : new Uint8Array(),
                    chains: this.#activeChain ? [this.#activeChain] : [],
                    features: ['iota:signAndExecuteTransaction'],
                }),
        );
    }

    constructor() {
        this.#events = mitt();
        this.#accounts = [];
        const { name, target } = generateWalletMessageStreamIdentifiers(NAME);
        this.#messagesStream = new WindowMessageStream(name, target);
        this.#messagesStream.messages.subscribe(({ payload }) => {
            if (isWalletStatusChangePayload(payload)) {
                const { network, accounts } = payload;
                if (network) {
                    this.#setActiveChain(network);
                    if (!accounts) {
                        // in case an accounts change exists skip updating chains of current accounts
                        // accounts will be updated in the if block below
                        this.#accounts = this.#accounts.map(
                            ({ address, features, icon, label, publicKey }) =>
                                new ReadonlyWalletAccount({
                                    address,
                                    publicKey,
                                    chains: this.#activeChain ? [this.#activeChain] : [],
                                    features,
                                    label,
                                    icon,
                                }),
                        );
                    }
                }
                if (accounts) {
                    this.#setAccounts(accounts);
                }
                this.#events.emit('change', { accounts: this.accounts });
            }
        });
    }

    #on: StandardEventsOnMethod = (event, listener) => {
        this.#events.on(event, listener);
        return () => this.#events.off(event, listener);
    };

    #connected = async () => {
        this.#setActiveChain(await this.#getActiveNetwork());
        if (!(await this.#hasPermissions(['viewAccount']))) {
            return;
        }
        const accounts = await this.#getAccounts();
        this.#setAccounts(accounts);
        if (this.#accounts.length) {
            this.#events.emit('change', { accounts: this.accounts });
        }
    };

    #connect: StandardConnectMethod = async (input) => {
        if (!input?.silent) {
            await mapToPromise(
                this.#send<AcquirePermissionsRequest, AcquirePermissionsResponse>({
                    type: 'acquire-permissions-request',
                    permissions: ALL_PERMISSION_TYPES,
                }),
                (response) => response.result,
            );
        }

        await this.#connected();

        return { accounts: this.accounts };
    };

    #signTransaction: IotaSignTransactionMethod = async ({ transaction, account, ...input }) => {
        return mapToPromise(
            this.#send<SignTransactionRequest, SignTransactionResponse>({
                type: 'sign-transaction-request',
                transaction: {
                    ...input,
                    // account might be undefined if previous version of adapters is used
                    // in that case use the first account address
                    account: account?.address || this.#accounts[0]?.address || '',
                    transaction: await transaction.toJSON(),
                },
            }),
            ({ result: { signature, bytes } }) => ({
                signature,
                bytes,
            }),
        );
    };

    #signAndExecuteTransaction: IotaSignAndExecuteTransactionMethod = async (input) => {
        return mapToPromise(
            this.#send<ExecuteTransactionRequest, ExecuteTransactionResponse>({
                type: 'execute-transaction-request',
                transaction: {
                    type: 'transaction',
                    data: await input.transaction.toJSON(),
                    options: {
                        showRawEffects: true,
                        showRawInput: true,
                    },
                    // account might be undefined if previous version of adapters is used
                    // in that case use the first account address
                    account: input.account?.address || this.#accounts[0]?.address || '',
                },
            }),
            ({ result: { rawEffects, rawTransaction, digest } }) => {
                const [
                    {
                        txSignatures: [signature],
                        intentMessage: { value: bcsTransaction },
                    },
                ] = bcs.SenderSignedData.parse(fromB64(rawTransaction!));

                const bytes = bcs.TransactionData.serialize(bcsTransaction).toBase64();

                return {
                    digest,
                    signature,
                    bytes,
                    effects: toB64(new Uint8Array(rawEffects!)),
                };
            },
        );
    };

    #signPersonalMessage: IotaSignPersonalMessageMethod = async ({ message, account }) => {
        return mapToPromise(
            this.#send<SignMessageRequest, SignMessageRequest>({
                type: 'sign-personal-message-request',
                args: {
                    message: toB64(message),
                    accountAddress: account.address,
                },
            }),
            (response) => {
                if (!response.return) {
                    throw new Error('Invalid sign message response');
                }
                return response.return;
            },
        );
    };

    #hasPermissions(permissions: HasPermissionsRequest['permissions']) {
        return mapToPromise(
            this.#send<HasPermissionsRequest, HasPermissionsResponse>({
                type: 'has-permissions-request',
                permissions: permissions,
            }),
            ({ result }) => result,
        );
    }

    #getAccounts() {
        return mapToPromise(
            this.#send<GetAccount, GetAccountResponse>({
                type: 'get-account',
            }),
            (response) => response.accounts,
        );
    }

    #getActiveNetwork() {
        return mapToPromise(
            this.#send<BasePayload, SetNetworkPayload>({
                type: 'get-network',
            }),
            ({ network }) => network,
        );
    }

    #setActiveChain({ network }: NetworkEnvType) {
        this.#activeChain =
            network === Network.Custom ? getCustomNetwork().chain : getNetwork(network).chain;
    }

    #send<RequestPayload extends Payload, ResponsePayload extends Payload | void = void>(
        payload: RequestPayload,
        responseForID?: string,
    ): Observable<ResponsePayload> {
        const msg = createMessage(payload, responseForID);
        this.#messagesStream.send(msg);
        return this.#messagesStream.messages.pipe(
            filter(({ id }) => id === msg.id),
            map((msg) => msg.payload as ResponsePayload),
        );
    }
}
