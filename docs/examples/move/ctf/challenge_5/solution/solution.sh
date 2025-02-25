# Step 1: run `cargo run` to print the expected pizza ingredients

cargo run


# Step 2: Call `cook` with the expected pizza ingredients

iota client call --package 0xa0fb6be5ee8585e7d512e73739657319dbfe3cb58817c062d0fd67335193fbd5 --module pizza --function cook --args 10 3 610 370 18 200 180 0


# Step 3: Call `get_flag` with the pizza object from step 2

iota client call --package 0xa0fb6be5ee8585e7d512e73739657319dbfe3cb58817c062d0fd67335193fbd5 --module pizza --function get_flag --args <pizza_object_from_step_2>