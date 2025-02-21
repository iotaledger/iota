module conventions::errors {
    // ✅ Correct
    const ENameHasMaxLengthOf64Chars: u64 = 0;

    // ❌ Incorrect
    const INVALID_NAME: u64 = 0;
}
