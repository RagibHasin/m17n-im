#!/bin/bash

# If there are new files with headers that can't match the conditions here,
# then the files can be ignored by an additional glob argument via the -g flag.
# For example:
#   -g "!src/special_file.rs"
#   -g "!src/special_directory"

# Check all the standard Rust source files
output=$(rg "^// SPDX-License-Identifier: MIT$\n\n" --files-without-match --multiline -g "*.rs" -g "!examples/bn-khipro/src/classic.rs" -g "!examples/bn-khipro/src/dark.rs" .)

if [ -n "$output" ]; then
	echo -e "The following files lack the correct copyright header:\n"
	echo $output
	echo -e "\n\nPlease add the following header:\n"
	echo "// SPDX-License-Identifier: MIT"
	echo -e "\n... rest of the file ...\n"
	exit 1
fi

echo "All files have correct copyright headers."
exit 0
