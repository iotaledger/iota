# Step 1: Create three pizza boxes with pineapple

iota client ptb --move-call 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29::pizza::cook 1 1 1 1 1 1 1 1 \
    --move-call 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29::pizza::cook 1 1 1 1 1 1 1 1 \
    --move-call 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29::pizza::cook 1 1 1 1 1 1 1 1


# Step 2: Transfer the pizza boxes to the PizzaBoxRecycler object, then call accept_box passing the pizza_box objects

iota client ptb --transfer-objects [<PizzaBox1>, <PizzaBox2>, <PizzaBox3>] @0x6b45253a27c915c0604e87c3959934d02c8c6d5304b24da344927a6d32d59b1e \
    --move-call 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29::recycle::accept_box @0x6b45253a27c915c0604e87c3959934d02c8c6d5304b24da344927a6d32d59b1e @<PizzaBox1> \
    --move-call 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29::recycle::accept_box @0x6b45253a27c915c0604e87c3959934d02c8c6d5304b24da344927a6d32d59b1e @<PizzaBox2> \
    --move-call 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29::recycle::accept_box @0x6b45253a27c915c0604e87c3959934d02c8c6d5304b24da344927a6d32d59b1e @<PizzaBox3> \


# Step 3: Call get_flag
iota client call --package 0x5196f5b912e20b9b7153c7f4426c902ddaad593fcb1125becc70f3904981ff29 --module recycle --function get_flag --args 0x6b45253a27c915c0604e87c3959934d02c8c6d5304b24da344927a6d32d59b1e