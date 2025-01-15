#!/bin/bash

# Set variables for the Sui repository (source) and IOTA repository (target)
REPO_PATH="$HOME/works/sui"  # Source repository (Sui)
TARGET_REPO_PATH="$HOME/works/iota"  # Target repository (IOTA)
BASE_COMMIT="fe8982b16c"  # Base commit in the Sui repository
TARGET_COMMIT="4dce691cea"  # Target commit in the Sui repository
PATCH_FILE="$TARGET_REPO_PATH/iota_changes.patch"  # Save patch file in the target repo

# Step 1: Generate the patch from the Sui repository
echo "Navigating to the Sui repository..."
cd "$REPO_PATH" || { echo "Sui repository path does not exist."; exit 1; }

# Verify the git repository
if [ ! -d ".git" ]; then
    echo "Not a valid Git repository: $REPO_PATH"
    exit 1
fi

# Generate the patch
echo "Generating patch from $BASE_COMMIT to $TARGET_COMMIT..."
git diff "$BASE_COMMIT" "$TARGET_COMMIT" > "$PATCH_FILE"

# Verify patch creation
if [ -f "$PATCH_FILE" ]; then
    echo "Patch file created successfully: $PATCH_FILE"
else
    echo "Failed to create the patch file."
    exit 1
fi

# List of crates replacements
declare -A replacements=(
    ["crates/mysten-common"]="crates/iota-common"
    ["crates/mysten-metrics"]="crates/iota-metrics"
    ["crates/mysten-network"]="crates/iota-network-stack"
    ["crates/mysten-util-mem"]="crates/iota-util-mem"
    ["crates/mysten-util-mem-derive"]="crates/iota-util-mem-derive"
)
echo "Applying replacements in the patch..."
for key in "${!replacements[@]}"; do
    sed -i "s/$key/${replacements[$key]}/g" "$PATCH_FILE"
done


# Replace 'sui' with 'iota' and 'Sui' with 'Iota'
echo "Replacing all occurrences of 'sui' with 'iota' and 'Sui' with 'Iota' in the patch..."
perl -pe 's/sui/iota/g; s/Sui/Iota/g' "$PATCH_FILE" > "${PATCH_FILE}.tmp"
mv "${PATCH_FILE}.tmp" "$PATCH_FILE"

# Step 2: Apply the patch to the IOTA repository
echo "Navigating to the IOTA repository..."
cd "$TARGET_REPO_PATH" || { echo "IOTA repository path does not exist."; exit 1; }

# Verify the git repository
if [ ! -d ".git" ]; then
    echo "Not a valid Git repository: $TARGET_REPO_PATH"
    exit 1
fi

# Apply the patch
echo "Applying the patch with fallback options..."
git apply --reject -v "$PATCH_FILE"

# Check for rejected hunks
if [ $? -ne 0 ]; then
    echo "Conflicts detected! Rejected hunks have been saved as .rej files."
    echo "You can manually resolve them or try applying manually using:"
    echo "  patch -p1 < $PATCH_FILE"
else
    echo "Patch applied successfully without conflicts."
fi

# Add all non-conflicting changes
echo "Staging all non-conflicting changes..."
git add -u

# Final message
echo "Non-conflicting changes have been staged."
echo "Please resolve conflicts in .rej files or use 'patch -p1' for manual application."