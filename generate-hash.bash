#!/bin/bash
# sha256 hash all files in ./Snowden archive/** to hashes-tmp.txt
# hash the generated hash file to hashes-merged.hash
# remove hashes-tmp.txt

set -e

# Generate SHA256 hashes for all files in Snowden archive directory
# Use a two-pass approach: first collect sorted filenames, then hash them
find "Snowden archive" -type f | LC_ALL=C sort > /tmp/files-to-hash.txt
while IFS= read -r file; do
    sha256sum "$file"
done < /tmp/files-to-hash.txt > hashes-tmp.txt
rm /tmp/files-to-hash.txt

# Hash the generated hash file
sha256sum hashes-tmp.txt > hashes-merged.hash

# Remove temporary hash file
rm hashes-tmp.txt

echo "Hash generation complete. Result saved to hashes-merged.hash"
