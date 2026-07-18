#!/bin/bash
# Build OpenWorld API for Alibaba Cloud Function Compute (Custom Runtime).
# Requires: cargo install cross  +  Docker running
#
# Usage:
#   chmod +x build-fc.sh
#   ./build-fc.sh
#
# Output: openworld-fc.zip  (upload this in FC console)

set -e

BINARY=api
TARGET=x86_64-unknown-linux-musl
OUTDIR=fc-package

echo "── Building OpenWorld for Function Compute ─────────────"
echo "  Target : $TARGET"
echo "  Binary : $BINARY"

# Cross-compile for Linux (requires Docker)
echo ""
echo "·  Compiling (this takes ~2 min first time)..."
cross build --bin $BINARY --target $TARGET --release

echo "✓  Compiled → target/$TARGET/release/$BINARY"

# Package bootstrap + binary into zip
rm -rf $OUTDIR && mkdir -p $OUTDIR
cp target/$TARGET/release/$BINARY $OUTDIR/$BINARY
cp bootstrap $OUTDIR/bootstrap
chmod +x $OUTDIR/$BINARY $OUTDIR/bootstrap

cd $OUTDIR
zip -r ../openworld-fc.zip .
cd ..

echo "✓  Packaged → openworld-fc.zip"
echo ""
echo "── Next steps ──────────────────────────────────────────"
echo "  1. FC Console → Create Function"
echo "     Runtime:  Custom Runtime"
echo "     Handler:  (leave blank — bootstrap handles it)"
echo "     Zip:      upload openworld-fc.zip"
echo ""
echo "  2. Set environment variables in FC Console:"
echo "     QWEN_API_KEY        = <your key>"
echo "     QWEN_MODEL          = qwen3.7-max"
echo "     QWEN_ENDPOINT       = https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions"
echo "     SERPAPI_KEY         = <your key>"
echo "     MAPBOX_ACCESS_TOKEN = <your key>"
echo "     OSS_BUCKET          = qwenhackkongphop"
echo "     OSS_ENDPOINT        = oss-ap-southeast-7.aliyuncs.com"
echo "     OSS_ACCESS_KEY_ID   = <your key>"
echo "     OSS_ACCESS_KEY_SECRET = <your key>"
echo "     SLS_PROJECT         = qwenhackkongphop"
echo "     SLS_LOGSTORE        = logkongphop"
echo "     SLS_ENDPOINT        = ap-southeast-7.log.aliyuncs.com"
echo "     SLS_ACCESS_KEY_ID   = <your key>"
echo "     SLS_ACCESS_KEY_SECRET = <your key>"
echo "     MEMORY_DIR          = /tmp/memory"
echo "     REPORTS_DIR         = /tmp/reports"
echo ""
echo "  3. Add HTTP Trigger → public URL → test:"
echo "     curl https://<fc-url>/health"
echo "────────────────────────────────────────────────────────"
