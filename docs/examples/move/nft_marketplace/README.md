# Marketplace Guide

The `nft_marketplace.move` module provides a straightforward implementation of a marketplace extension. To utilize it, follow the steps outlined below.
The `item_for_market.move` contains mocked item data for use within the marketplace.
The `rental_extension.move` is an extension adds functionality to enable item rentals.

## Steps to Use the Marketplace

### 1. Connect to the Network

Connect to the IOTA network (e.g., using a faucet to obtain tokens).

### 2. Install Kiosk

By installation, we mean creating a Kiosk object and an OwnerCap, then transferring them to the caller.
Run the following command to install the Kiosk module:

```bash
iota client call \                                   
    --package 0x2 \
    --module kiosk \
    --function default
```

### 3. Publish `nft_marketplace` package

#### 3.1(Optional) Publish Kiosk rules modules if these are not present in the network you are using

Publish Kiosk rules modules(package):

```bash
iota client publish iota/kiosk
```

After publishing, export the following variable:

- `RULES_PACKAGE_ID`: The ID of rules package.

#### 3.2 Publish marketplace extension

Publish the nft_marketplace.move module:

```bash
iota client publish iota/docs/examples/move/nft_marketplace
```

After publishing, export the following variables:

- `MARKETPLACE_PACKAGE_ID`: The ID of whole marketplace package.
- `MARKETPLACE_PUBLISHER_ID`: The ID of the publisher object created during marketplace package publishing."

### 4. Create an Clothing Store Item

Create an item, for instance:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module clothing_store \
    --function new_jeans
```

After creation, export the following variable:

- `CLOTHING_STORE_ITEM_ID`: The ID of the published item (in this case, Jeans).

### 5. Create a Transfer Policy

`TransferPolicy` is a generic shared object acting as a central authority enforcing everyone to check their purchase is valid against the defined policy before the purchased item is transferred to the buyers. Object is specified by concrete type.
`default` function creates `TransferPolicy` object and an OwnerCap, then transferring them to the caller.

Set up a transfer policy for the created item using the command:

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

The install function enables installation of the Marketplace extension in a kiosk.
Under the hood it invokes `kiosk_extension::add` that adds extension to the Kiosk via dynamic field.
Install the marketplace extension on the created kiosk using the command:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module nft_marketplace \
    --function install \
    --args $KIOSK_ID $KIOSK_CAP_ID
```

### 7. Set a Price for the Item

Set the price for the item:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module nft_marketplace \
    --function set_price \
    --args $KIOSK_ID $KIOSK_CAP_ID $CLOTHING_STORE_ITEM_ID 50000 \
    --type-args "$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans"
```

### 8.(Optional) Set Royalties

Royalties are a percentage of the item's price or revenue paid to the owner for the use or sale of their asset.

Set royalties for the item:

```bash
iota client call \
    --package $MARKETPLACE_PACKAGE_ID \
    --module nft_marketplace \
    --function setup_royalties \
    --args $ITEM_TRANS_POLICY $ITEM_TRANS_POLICY_CAP 5000 2000 \
    --type-args "$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans"
```

### 9. Buy an Item:

#### 9.1 Get the Item Price:

```bash
iota client ptb \
--move-call $MARKETPLACE_PACKAGE_ID::nft_marketplace::get_item_price "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$CLOTHING_STORE_ITEM_ID --assign item_price \
```

#### 9.2(Optional) Calculate rolyalties of the Item:

```bash
--move-call $RULES_PACKAGE_ID::royalty_rule::fee_amount "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$ITEM_TRANS_POLICY item_price --assign royalties_amount \
```

#### 9.3 Create a payment coin with a specific amount (price + optional royalties):

```bash
--split-coins gas "[item_price, royalties_amount]" --assign payment_coins \
--merge-coins payment_coins.0 "[payment_coins.1]" \
```

#### 9.4 Buy an Item using `payment_coins.0`:

Here, when we buy an item, we pay the owner the item's price. If the royalty rule is enabled, an additional royalty fee, calculated as a percentage of the initial item price, is also paid. Once both payments are completed, the item is ready for transferring to the buyer.

To purchase the item:

```bash
--move-call $MARKETPLACE_PACKAGE_ID::nft_marketplace::buy_item "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$ITEM_TRANS_POLICY @$CLOTHING_STORE_ITEM_ID payment_coins.0 --assign purchased_item
```

#### 9.5 Transfer an Item to the buyer:

```bash
--move-call 0x2::transfer::public_transfer "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" purchased_item @<buyer address> \
```

The final purchase PTB request, including royalties, should look like this:

```bash
iota client ptb \
--move-call $MARKETPLACE_PACKAGE_ID::nft_marketplace::get_item_price "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$CLOTHING_STORE_ITEM_ID --assign item_price \
--move-call $RULES_PACKAGE_ID::royalty_rule::fee_amount "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$ITEM_TRANS_POLICY item_price --assign royalties_amount \
--split-coins gas "[item_price, royalties_amount]" --assign payment_coins \
--merge-coins payment_coins.0 "[payment_coins.1]" \
--move-call $MARKETPLACE_PACKAGE_ID::nft_marketplace::buy_item "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" @$KIOSK_ID @$ITEM_TRANS_POLICY @$CLOTHING_STORE_ITEM_ID payment_coins.0 --assign purchased_item \
--move-call 0x2::transfer::public_transfer "<$MARKETPLACE_PACKAGE_ID::clothing_store::Jeans>" purchased_item @<buyer address>
```
