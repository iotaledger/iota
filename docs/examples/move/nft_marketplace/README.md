# Marketplace Extension Usage

## Modules

The [`marketplace_extension.move`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/marketplace_extension.move)
module provides a straightforward implementation of a marketplace extension. To use it, follow the
[steps outlined below](#how-to-use-the-marketplace).

The [`clothing_store.move`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/clothing_store.move) module contains mocked item data for
use within the marketplace.

The [`rental_extension.move`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/rental_extension.move) module adds the functionality to enable item rentals.

## How To Use the Marketplace

### 1. Install the IOTA CLI and Connect to the Network

The first thing you'll need to do is [install the IOTA CLI](https://docs.iota.org/developer/getting-started/install-iota), [connect to an IOTA network](https://docs.iota.org/developer/getting-started/connect) and [get some test tokens](https://docs.iota.org/developer/getting-started/get-coins) to pay for [gas](https://docs.iota.org/about-iota/tokenomics/gas-in-iota).

### 2. Install Kiosk

You can install the Kiosk by creating a `Kiosk` object, which will also create its `OwnerCap`, and then transferring
them to the caller.

Run the following command to install the Kiosk module:

```bash
iota client call \
    --package 0x2 \
    --module kiosk \
    --function default
```

After publishing, export the following variables:

- `KIOSK_ID`: The ID of the installed Kiosk object.
- `KIOSK_CAP_ID`: The ID of the installed Kiosk's owner cap

### 3. Publish `nft_marketplace` package

#### 3.1(Optional) Publish Kiosk rules modules if these are not present in the network you are using

You can publish Kiosk rules modules(package) using the following command:

```bash
iota client publish $IOTA_REPO_DIR/kiosk
```

After publishing, export the following variable:

- `RULES_PACKAGE_ID`: The ID of the rules package.

#### 3.2 Publish the `nft_marketplace` Package

```bash
iota client publish $IOTA_REPO_DIR/docs/examples/move/nft_marketplace
```

After publishing, export the following variables:

- `MARKETPLACE_PACKAGE_ID`: The ID of the whole marketplace package.
- `MARKETPLACE_PUBLISHER_ID`: The ID of the publisher object created during marketplace package publishing."

### 4. Create a Clothing Store Item

Next, you should use the functions in the `clothing_store` module to create an item, for instance:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module clothing_store \
    --function new_jeans
```

After creation, export the following variable:

- `CLOTHING_STORE_ITEM_ID`: The ID of the published item (in this case, Jeans).

### 5. Create a Transfer Policy

`TransferPolicy` is a generic-shared object that acts as a central authority enforcing that everyone checks their
purchase is valid against the defined policy before the purchased item is transferred to the buyer. The object is
specified by concrete type:
The `default` function creates a `TransferPolicy` object and a `TransferPolicyCap`, then transfers them to the caller.

The `TransferPolicyCap` object serves as proof of ownership of the `TransferPolicy` object.
A capability granting the owner permission to `add/remove` rules, `withdraw`, and `destroy_and_withdraw` the `TransferPolicy`.

You can set up a transfer policy for the created item using the following command:

```bash
iota client call \
    --package 0x2 \
    --module transfer_policy \
    --function default \
    --gas-budget 10000000 \
    --args $MARKETPLACE_PUBLISHER_ID \
    --type-args "$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans"
```

After publishing, export the following variables:

- `ITEM_TRANS_POLICY`: The ID of the item transfer policy object.
- `ITEM_TRANS_POLICY_CAP`: The ID of the item transfer policy object owner capability"

### 6. Install the Extension on the Kiosk

The [`install`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/marketplace_extension.move#L39-L45) function enables the installation of the Marketplace extension in a kiosk.
Under the hood, it invokes `kiosk_extension::add`, which adds an extension to the Kiosk via a [dynamic field](https://docs.iota.org/developer/iota-101/objects/dynamic-fields/).
You can install the marketplace extension on the created kiosk using the following command:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module marketplace_extension \
    --function install \
    --args $KIOSK_ID $KIOSK_CAP_ID
```

### 7. Set a Price for the Item

You can use the [`set_price`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/marketplace_extension.move#L98-L114) function to set the price for the item:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module marketplace_extension \
    --function set_price \
    --args $KIOSK_ID $KIOSK_CAP_ID $CLOTHING_STORE_ITEM_ID 50000 \
    --type-args "$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans"
```

### 8.(Optional) Set Royalties

Royalties are a percentage of the item's price or revenue paid to the owner for using or selling their asset.

You can use the [`set_royalties`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/marketplace_extension.move#L58-L60) function to set royalties for the item:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module marketplace_extension \
    --function setup_royalties \
    --args $ITEM_TRANS_POLICY $ITEM_TRANS_POLICY_CAP 5000 2000 \
    --type-args "$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans"
```

### 9. Buy an Item

#### 9.1 Get the Item Price

You can use the following [Programmable Transaction Block](https://docs.iota.org/developer/iota-101/transactions/ptb/programmable-transaction-blocks-overview) to call the
[`get_item_price`](https://github.com/iotaledger/iota/blob/develop/docs/examples/move/nft_marketplace/sources/marketplace_extension.move#L116-L127)
and assign it to an `item_price` variable. In this case, the Jeans item:

```bash
iota client ptb \
--move-call $MARKETPLACE_PACKAGE_ID::marketplace_extension::get_item_price "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$CLOTHING_STORE_ITEM_ID --assign item_price \
```

#### 9.2(Optional) Calculate the Royalties For the Item

You can use the following [move-call](https://docs.iota.org/references/cli/ptb#move-call) to get the royalties for any given product by calling the `kiosk::royalty_rule::fee_amount` function
and assign it to a `royalties_amount` variable. In this case, the Jeans item:

```bash
--move-call $RULES_PACKAGE_ID::royalty_rule::fee_amount "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$ITEM_TRANS_POLICY item_price --assign royalties_amount \
```

#### 9.3 Create a Payment Coin With a Specific Amount (Price + Optional Royalties)

You can use the following command to [split your gas tokens](https://docs.iota.org/references/cli/ptb#split-destroy-and-merge-coins) to pay for the item's price and royalties:

```bash
--split-coins gas "[item_price, royalties_amount]" --assign payment_coins \
--merge-coins payment_coins.0 "[payment_coins.1]" \
```

#### 9.4 Buy an Item Using `payment_coins.0`

You can use the following [move-call](https://docs.iota.org/references/cli/ptb#move-call) to pay the owner the item's price.
If the royalty rule is enabled, an additional royalty fee, calculated as a percentage of the initial item price, is also
paid.
Once both payments are completed, the item is ready for transfer to the buyer.

To purchase the item:

```bash
--move-call $MARKETPLACE_PACKAGE_ID::marketplace_extension::buy_item "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$ITEM_TRANS_POLICY @$CLOTHING_STORE_ITEM_ID payment_coins.0 --assign purchased_item
```

#### 9.5 Transfer an Item to the Buyer

Finally, you can set up the
[public_transfer](https://docs.iota.org/references/framework/iota/transfer#function-public_transfer) to
transfer the purchased item to the buyer:

```bash
--move-call 0x2::transfer::public_transfer "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" purchased_item @<buyer address> \
```

You can combine all the previous steps into one purchase
[PTB](https://docs.iota.org/developer/iota-101/transactions/ptb/programmable-transaction-blocks-overview) request,
including royalties, which should look like this:

```bash
iota client ptb \
--move-call $MARKETPLACE_PACKAGE_ID::marketplace_extension::get_item_price "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$CLOTHING_STORE_ITEM_ID --assign item_price \
--move-call $RULES_PACKAGE_ID::royalty_rule::fee_amount "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$ITEM_TRANS_POLICY item_price --assign royalties_amount \
--split-coins gas "[item_price, royalties_amount]" --assign payment_coins \
--merge-coins payment_coins.0 "[payment_coins.1]" \
--move-call $MARKETPLACE_PACKAGE_ID::marketplace_extension::buy_item "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$ITEM_TRANS_POLICY @$CLOTHING_STORE_ITEM_ID payment_coins.0 --assign purchased_item \
--move-call 0x2::transfer::public_transfer "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" purchased_item @<buyer address>
```
