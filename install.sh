#!/bin/sh

set -e

rw_cli_install="${HOME}/.redwoodjs"
bin_dir="$rw_cli_install/bin"
exe="$bin_dir/rw"

if [ ! -d "$bin_dir" ]; then
	mkdir -p "$bin_dir"
fi

# Get os and arch but ensure they're lowercase to match the S3 bucket keys
os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m | tr '[:upper:]' '[:lower:]')

if [ "${RW_CLI_PROGRESS_BAR:-1}" -eq 0 ]; then
    # curl --fail --location --no-progress-meter --output "$exe" "https://rw-cli-releases.s3.us-west-1.amazonaws.com/$os/$arch/rw"
    curl --fail --location --no-progress-meter --output "$exe" "https://tobbe.dev/$os/$arch/rw"
else
    # curl --fail --location --progress-bar --output "$exe" "https://rw-cli-releases.s3.us-west-1.amazonaws.com/$os/$arch/rw"
    curl --fail --location --progress-bar --output "$exe" "https://tobbe.dev/$os/$arch/rw"
fi

chmod +x "$exe"

echo "The RedwoodJS CLI was successfully installed to $exe"

if command -v rw >/dev/null; then
    echo "Run 'rw --help' to get started"
else
    case $SHELL in
    /bin/zsh) shell_profile=".zshrc" ;;
    *) shell_profile=".bash_profile" ;;
    esac
    echo "Manually add the directory to your \$HOME/$shell_profile (or similar)"
    echo "  export RW_CLI_INSTALL=\"$rw_cli_install\""
    echo "  export PATH=\"\$RW_CLI_INSTALL/bin:\$PATH\""
    echo "Run '$exe --help' to get started"
fi
