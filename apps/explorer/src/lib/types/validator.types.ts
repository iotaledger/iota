import { type IotaValidatorSummary } from '@iota/iota-sdk/client';

export type IotaValidatorSummaryExtended = IotaValidatorSummary & { isPending?: boolean };
