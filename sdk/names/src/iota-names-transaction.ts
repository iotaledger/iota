// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';
import type {
    Transaction,
    TransactionObjectArgument,
    TransactionObjectInput,
} from '@iota/iota-sdk/transactions';
import { IOTA_CLOCK_OBJECT_ID, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { ALLOWED_METADATA } from './constants.js';
import { isNestedSubname, isSubname } from './helpers.js';
import type { IotaNamesClient } from './iota-names-client.js';
import type {
    ReceiptParams,
    RegistrationParams,
    RegistrationWithYearsParams,
    RenewalParams,
} from './types.js';
import { isValidIotaName, normalizeIotaName } from './utils.js';

export class IotaNamesTransaction {
    iotaNamesClient: IotaNamesClient;
    transaction: Transaction;

    constructor(client: IotaNamesClient, transaction: Transaction) {
        this.iotaNamesClient = client;
        this.transaction = transaction;
    }

    /**
     * Registers a name.
     */
    async register(params: RegistrationParams): Promise<TransactionObjectArgument> {
        const paymentIntent = this.initRegistration(params.name);

        const couponCodes = params.couponCodes;
        let discountedPrice: number | null = null;

        if (couponCodes && couponCodes.length > 0) {
            discountedPrice = await this.iotaNamesClient.calculateDiscountedPrice({
                coupons: couponCodes,
                name: params.name,
                years: 1,
                isRegistration: true,
                address: params.address,
            });

            for (const couponCode of couponCodes) {
                this.applyCoupon(couponCode, paymentIntent);
            }
        }

        const amounts = [discountedPrice ?? this.getBasePrice(paymentIntent)];
        const payment = this.transaction.splitCoins(this.transaction.object(params.coin), amounts);

        const receipt = this.generateReceipt({
            paymentIntent,
            payment,
            coinConfig: params.coinConfig || { type: IOTA_TYPE_ARG },
        });

        return this.finalizeRegister(receipt);
    }

    /**
     * Registers a name for a given amount of years.
     */
    async registerWithYears(
        params: RegistrationWithYearsParams,
    ): Promise<TransactionObjectArgument> {
        const paymentIntent = this.initRegistrationWithYears(params.name, params.years);

        const couponCodes = params.couponCodes;
        let discountedPrice: number | null = null;

        if (couponCodes && couponCodes.length > 0) {
            discountedPrice = await this.iotaNamesClient.calculateDiscountedPrice({
                coupons: couponCodes,
                name: params.name,
                years: params.years,
                isRegistration: true,
                address: params.address,
            });

            for (const couponCode of couponCodes) {
                this.applyCoupon(couponCode, paymentIntent);
            }
        }

        const amounts = [discountedPrice ?? this.getBasePrice(paymentIntent)];
        const payment = this.transaction.splitCoins(this.transaction.object(params.coin), amounts);

        const receipt = this.generateReceipt({
            paymentIntent,
            payment,
            coinConfig: params.coinConfig || { type: IOTA_TYPE_ARG },
        });

        return this.finalizeRegister(receipt);
    }

    /**
     * Renews an NFT for a number of years.
     */
    async renew(params: RenewalParams): Promise<void> {
        const paymentIntent = this.initRenewal(params.nft, params.years);

        const couponCodes = params.couponCodes;
        let discountedPrice: number | null = null;

        if (couponCodes && couponCodes.length > 0) {
            discountedPrice = await this.iotaNamesClient.calculateDiscountedPrice({
                coupons: couponCodes,
                name: params.name,
                years: params.years,
                isRegistration: false,
                address: params.address,
            });

            for (const couponCode of couponCodes) {
                this.applyCoupon(couponCode, paymentIntent);
            }
        }

        const amounts = [discountedPrice ?? this.getBasePrice(paymentIntent)];
        const payment = this.transaction.splitCoins(this.transaction.object(params.coin), amounts);

        const receipt = this.generateReceipt({
            paymentIntent,
            payment,
            coinConfig: params.coinConfig || { type: IOTA_TYPE_ARG },
        });
        this.finalizeRenew(receipt, params.nft);
    }

    initRegistration(name: string): TransactionObjectArgument {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = this.iotaNamesClient.getPackage('packageId');
        return this.transaction.moveCall({
            target: `${packageId}::payment::init_registration`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.pure.string(name),
            ],
        });
    }

    initRegistrationWithYears(name: string, years: number): TransactionObjectArgument {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = this.iotaNamesClient.getPackage('packageId');
        return this.transaction.moveCall({
            target: `${packageId}::payment::init_registration_with_years`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.pure.string(name),
                this.transaction.pure.u8(years),
            ],
        });
    }

    initRenewal(nft: TransactionObjectInput, years: number): TransactionObjectArgument {
        const packageId = this.iotaNamesClient.getPackage('packageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        return this.transaction.moveCall({
            target: `${packageId}::payment::init_renewal`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.pure.u8(years),
            ],
        });
    }

    handleBasePayment(
        paymentIntent: TransactionObjectArgument,
        payment: TransactionObjectArgument,
        paymentType: string,
    ): TransactionObjectArgument {
        const paymentsPackageId = this.iotaNamesClient.getPackage('paymentsPackageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        return this.transaction.moveCall({
            target: `${paymentsPackageId}::payments::handle_base_payment`,
            arguments: [this.transaction.object(iotaNamesObjectId), paymentIntent, payment],
            typeArguments: [paymentType],
        });
    }

    finalizeRegister(receipt: TransactionObjectArgument): TransactionObjectArgument {
        const packageId = this.iotaNamesClient.getPackage('packageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        return this.transaction.moveCall({
            target: `${packageId}::payment::register`,
            arguments: [
                receipt,
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object.clock(),
            ],
        });
    }

    finalizeRenew(
        receipt: TransactionObjectArgument,
        nft: TransactionObjectInput,
    ): TransactionObjectArgument {
        const packageId = this.iotaNamesClient.getPackage('packageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        return this.transaction.moveCall({
            target: `${packageId}::payment::renew`,
            arguments: [
                receipt,
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.object.clock(),
            ],
        });
    }

    getBasePrice(paymentIntent: TransactionObjectArgument): TransactionObjectArgument {
        const packageId = this.iotaNamesClient.getPackage('packageId');

        return this.transaction.moveCall({
            target: `${packageId}::payment::request_base_amount`,
            arguments: [paymentIntent],
        });
    }

    applyCoupon(couponCode: string, paymentIntent: TransactionObjectArgument) {
        const couponsPackageId = this.iotaNamesClient.getPackage('couponsPackageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        return this.transaction.moveCall({
            target: `${couponsPackageId}::coupon_house::apply_coupon`,
            arguments: [
                paymentIntent,
                this.transaction.object(iotaNamesObjectId),
                this.transaction.pure.string(couponCode),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
            ],
        });
    }

    generateReceipt(params: ReceiptParams): TransactionObjectArgument {
        const receipt = this.handleBasePayment(
            params.paymentIntent,
            params.payment,
            params.coinConfig.type,
        );
        return receipt;
    }

    /**
     * Creates a subname.
     */
    createSubname({
        parentNft,
        name,
        expirationTimestampMs,
        allowChildCreation,
        allowTimeExtension,
    }: {
        parentNft: TransactionObjectInput;
        name: string;
        expirationTimestampMs: number;
        allowChildCreation: boolean;
        allowTimeExtension: boolean;
    }) {
        if (!isValidIotaName(name)) throw new Error('Invalid IOTA names');
        const isParentSubname = isNestedSubname(name);
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const subnamesPackageId = !isParentSubname
            ? this.iotaNamesClient.getPackage('subnamesPackageId')
            : null;
        const tempSubnameProxyPackageId = isParentSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        const subNft = this.transaction.moveCall({
            target: isParentSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::new`
                : `${subnamesPackageId}::subnames::new`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(parentNft),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
                this.transaction.pure.string(normalizeIotaName(name, 'dot')),
                this.transaction.pure.u64(expirationTimestampMs),
                this.transaction.pure.bool(!!allowChildCreation),
                this.transaction.pure.bool(!!allowTimeExtension),
            ],
        });

        return subNft;
    }

    /**
     * Builds the PTB to create a leaf subname.
     * Parent can be a `NameRegistration` or a `SubnameRegistration` object.
     * Can be passed in as an ID or a TransactionArgument.
     */
    createLeafSubname({
        parentNft,
        name,
        targetAddress,
    }: {
        parentNft: TransactionObjectInput;
        name: string;
        targetAddress: string;
    }) {
        if (!isValidIotaName(name)) throw new Error('Invalid IOTA names');
        const isParentSubname = isNestedSubname(name);

        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const subnamesPackageId = !isParentSubname
            ? this.iotaNamesClient.getPackage('subnamesPackageId')
            : null;
        const tempSubnameProxyPackageId = isParentSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        this.transaction.moveCall({
            target: isParentSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::new_leaf`
                : `${subnamesPackageId}::subnames::new_leaf`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(parentNft),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
                this.transaction.pure.string(normalizeIotaName(name, 'dot')),
                this.transaction.pure.address(targetAddress),
            ],
        });
    }

    /**
     * Removes a leaf subname.
     */
    removeLeafSubname({ parentNft, name }: { parentNft: TransactionObjectInput; name: string }) {
        if (!isValidIotaName(name)) throw new Error('Invalid IOTA names');
        const isParentSubname = isNestedSubname(name);
        if (!isSubname(name)) throw new Error('This can only be invoked for subnames');

        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const subnamesPackageId = !isParentSubname
            ? this.iotaNamesClient.getPackage('subnamesPackageId')
            : null;
        const tempSubnameProxyPackageId = isParentSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        this.transaction.moveCall({
            target: isParentSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::remove_leaf`
                : `${subnamesPackageId}::subnames::remove_leaf`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(parentNft),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
                this.transaction.pure.string(normalizeIotaName(name, 'dot')),
            ],
        });
    }

    /**
     * Sets the target address of an NFT.
     */
    setTargetAddress({
        nft, // Can be string or argument
        address,
        isSubname,
    }: {
        nft: TransactionObjectInput;
        address?: string;
        isSubname?: boolean;
    }) {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = !isSubname ? this.iotaNamesClient.getPackage('packageId') : null;
        const tempSubnameProxyPackageId = isSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        this.transaction.moveCall({
            target: isSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::set_target_address`
                : `${packageId}::controller::set_target_address`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.pure(bcs.option(bcs.Address).serialize(address).toBytes()),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
            ],
        });
    }

    /**
     * Sets a public name for the user.
     */
    setPublic(name: string) {
        if (!isValidIotaName(name)) throw new Error('Invalid IOTA names');
        const packageId = this.iotaNamesClient.getPackage('packageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        this.transaction.moveCall({
            target: `${packageId}::controller::set_reverse_lookup`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.pure.string(normalizeIotaName(name, 'dot')),
            ],
        });
    }

    /**
     * Unsets a Public name for the user.
     */
    unsetPublic() {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = this.iotaNamesClient.getPackage('packageId');

        this.transaction.moveCall({
            target: `${packageId}::controller::unset_reverse_lookup`,
            arguments: [this.transaction.object(iotaNamesObjectId)],
        });
    }

    /**
     * Edits the setup of a subname.
     */
    editSetup({
        parentNft,
        name,
        allowChildCreation,
        allowTimeExtension,
    }: {
        parentNft: TransactionObjectInput;
        name: string;
        allowChildCreation: boolean;
        allowTimeExtension: boolean;
    }) {
        if (!isValidIotaName(name)) throw new Error('Invalid IOTA names');
        const isParentSubname = isNestedSubname(name);

        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const subnamesPackageId = !isParentSubname
            ? this.iotaNamesClient.getPackage('subnamesPackageId')
            : null;
        const tempSubnameProxyPackageId = isParentSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        this.transaction.moveCall({
            target: isParentSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::edit_setup`
                : `${subnamesPackageId}::subnames::edit_setup`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(parentNft),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
                this.transaction.pure.string(normalizeIotaName(name, 'dot')),
                this.transaction.pure.bool(!!allowChildCreation),
                this.transaction.pure.bool(!!allowTimeExtension),
            ],
        });
    }

    /**
     * Extends the expiration of a subname.
     */
    extendExpiration({
        nft,
        expirationTimestampMs,
    }: {
        nft: TransactionObjectInput;
        expirationTimestampMs: number;
    }) {
        const subnamesPackageId = this.iotaNamesClient.getPackage('subnamesPackageId');
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');

        this.transaction.moveCall({
            target: `${subnamesPackageId}::subnames::extend_expiration`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.pure.u64(expirationTimestampMs),
            ],
        });
    }

    /**
     * Sets the user data of an NFT.
     */
    setUserData({
        nft,
        value,
        key,
        isSubname,
    }: {
        nft: TransactionObjectInput;
        value: string;
        key: string;
        isSubname?: boolean;
    }) {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = !isSubname ? this.iotaNamesClient.getPackage('packageId') : null;
        const tempSubnameProxyPackageId = isSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        if (!Object.values(ALLOWED_METADATA).some((x) => x === key)) throw new Error('Invalid key');

        this.transaction.moveCall({
            target: isSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::set_user_data`
                : `${packageId}::controller::set_user_data`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.pure.string(key),
                this.transaction.pure.string(value),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
            ],
        });
    }

    /**
     * Unsets the user data of an NFT.
     */
    unsetUserData({
        nft,
        key,
        isSubname,
    }: {
        nft: TransactionObjectInput;
        key: string;
        isSubname?: boolean;
    }) {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = !isSubname ? this.iotaNamesClient.getPackage('packageId') : null;
        const tempSubnameProxyPackageId = isSubname
            ? this.iotaNamesClient.getPackage('tempSubnameProxyPackageId')
            : null;

        if (!Object.values(ALLOWED_METADATA).some((x) => x === key)) throw new Error('Invalid key');

        this.transaction.moveCall({
            target: isSubname
                ? `${tempSubnameProxyPackageId}::subname_proxy::unset_user_data`
                : `${packageId}::controller::unset_user_data`,
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.pure.string(key),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
            ],
        });
    }

    /**
     * Burns an expired NFT to collect storage rebates.
     */
    burnExpired({ nft, isSubname }: { nft: TransactionObjectInput; isSubname?: boolean }) {
        const iotaNamesObjectId = this.iotaNamesClient.getPackage('iotaNamesObjectId');
        const packageId = this.iotaNamesClient.getPackage('packageId');

        this.transaction.moveCall({
            target: `${packageId}::controller::${
                isSubname ? 'burn_expired_subname' : 'burn_expired'
            }`, // Update this
            arguments: [
                this.transaction.object(iotaNamesObjectId),
                this.transaction.object(nft),
                this.transaction.object(IOTA_CLOCK_OBJECT_ID),
            ],
        });
    }
}
