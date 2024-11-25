# Marketplace Guide

The `nft_marketplace.move` module provides a straightforward implementation of a marketplace extension. To utilize it, follow the steps outlined below.
The `item_for_market.move` contains mocked item data for use within the marketplace.
The `rental_extension.move` is an extension adds functionality to enable item rentals.


## Steps to Use the Marketplace

### 1. Connect to the Network
Connect to the IOTA network (e.g., using a faucet to obtain tokens).

### 2. Install Kiosk
Run the following command to install the Kiosk module:

```bash
iota client call \                                   
    --package 0x2 \
    --module kiosk \
    --function default
```

### 3. Publish `item_for_market.move`

```bash
iota client publish
```

### 4. Create an Item

Create an item, for instance:

```bash
iota client call \
    --package $M_ITEMS_ID \
    --module market_items \
    --function new_jeans
```

After creation, export the following variables:

- PACKAGE_ID
- ITEM_ID
- PUBLISHER_ID

### 5. Create a Transfer Policy

Set up a transfer policy for the created item using the command:
```bash
iota client call \                                   
    --package 0x2 \
    --module transfer_policy \
    --function default \
    --gas-budget 10000000 \
    --args $PUBLISHER_ID \
    --type-args "$ITEM_FOR_MARKET_PACKAGE_ID::market_items::Jeans"
```

### 6. Publish rules and marketplace extension

Publish Kiosk rules modules:
```bash
iota client publish iota/kiosk/Move.toml
```

Publish the nft_marketplace.move module:
```bash
iota client publish iota/docs/examples/move/nft_marketplace/sources/nft_marketplace.move`
```

### 7. Install the Extension on the Kiosk

Install the marketplace extension on the created kiosk using the command:
```bash
iota client call \
    --package $MARKETPLACE_ID \
    --module nft_marketplace \
    --function install \
    --args $KIOSK_ID $KIOSK_CAP_ID
```

### 8. Set a Price for the Item

Set the price for the item:
```bash
iota client call \
    --package $MARKETPLACE_ID \
    --module nft_marketplace \
    --function set_price \
    --args $KIOSK_ID $KIOSK_CAP_ID $ITEM_ID 50000 \
    --type-args "&ITEM_FOR_MARKET_PACKAGE_ID::market_items::Jeans"

```

### 9.(Optional) Set Royalties

Set royalties for the item:

```bash
iota client call \
    --package $MARKETPLACE \
    --module nft_marketplace \
    --function setup_royalties \
    --args $ITEM_TRANS_POLICY_ID $ITEM_TRANS_POLICY_CAP_ID 5000 2000 \
    --type-args "ITEM_FOR_MARKET_PACKAGE_ID::market_items::Jeans"
```

### 10. Buy an Item:

To purchase the item:
```bash
iota client call \
    --package $MARKETPLACE \
    --module nft_marketplace \
    --function buy_item \
    --args $KIOSK_ID $ITEM_TRANS_POLICY_ID &ITEM_ID $COIN_ID \
    --type-args "ITEM_FOR_MARKET_PACKAGE_ID::market_items::Jeans"

```