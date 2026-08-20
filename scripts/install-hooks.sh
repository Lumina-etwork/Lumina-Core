#!/bin/bash
set -e

HOOKS_DIR=".git/hooks"

if [ ! -d "$HOOKS_DIR" ]; then
    echo "Error: .git/hooks directory not found. Make sure you are in the project root."
    exit 1
fi

cp scripts/pre-commit $HOOKS_DIR/pre-commit
chmod +x $HOOKS_DIR/pre-commit

echo "Pre-commit hook installed successfully!"
