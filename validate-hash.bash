#!/bin/bash
# Validate the integrity of Snowden archive files against hashes-merged.hash

set -e

# Check if hashes-merged.hash exists
if [ ! -f "hashes-merged.hash" ]; then
    echo "Error: hashes-merged.hash not found. Run generate-hash.bash first."
    exit 1
fi

# Generate current hashes
# Use a two-pass approach: first collect sorted filenames, then hash them
find "Snowden archive" -type f | LC_ALL=C sort > /tmp/files-to-hash.txt
while IFS= read -r file; do
    sha256sum "$file"
done < /tmp/files-to-hash.txt > hashes-tmp.txt
rm /tmp/files-to-hash.txt

# Hash the current hash file
CURRENT_HASH=$(sha256sum hashes-tmp.txt | awk '{print $1}')
EXPECTED_HASH=$(awk '{print $1}' hashes-merged.hash)

# Clean up temporary file
rm hashes-tmp.txt

# Compare hashes
if [ "$CURRENT_HASH" = "$EXPECTED_HASH" ]; then
    echo " Validation successful: Archive integrity verified"
    exit 0
else
    echo " Validation failed: Archive has been modified"
    echo "Expected: $EXPECTED_HASH"
    echo "Current:  $CURRENT_HASH"
    exit 1
fi
