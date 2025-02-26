# Steps: Create a ptb that calls get_ingredients, then make_dough, then get_flag.
# Note: we need to use assign to store the ingredients in a variable, then use that variable to make the dough.
# Finally, we need to use the dough variable to get the flag.

iota client ptb --move-call 0x7661363fdfe4cd602711244a8501c17e756a5304bcce0918efcef4af98c09291::ptb::get_ingredients --assign ingredients \
    --move-call 0x7661363fdfe4cd602711244a8501c17e756a5304bcce0918efcef4af98c09291::ptb::make_dough ingredients.0 ingredients.1 ingredients.2 ingredients.3 \
    --assign dough \
    --move-call 0x7661363fdfe4cd602711244a8501c17e756a5304bcce0918efcef4af98c09291::ptb::get_flag dough