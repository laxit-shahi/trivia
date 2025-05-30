#!/bin/bash

# Helper script to rebuild frontend for ngrok access
# Usage: ./build-for-ngrok.sh <backend_ngrok_url>

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 <backend_ngrok_url>"
    echo "Example: $0 https://abc123.ngrok.io"
    exit 1
fi

BACKEND_URL=$1

echo "🔧 Building frontend for ngrok access..."
echo "Backend URL: $BACKEND_URL"

cd frontend

# Set environment variables and build
REACT_APP_API_BASE_URL="$BACKEND_URL/api" \
REACT_APP_WS_URL="${BACKEND_URL/http/ws}/ws" \
npm run build

echo "✅ Build complete!"
echo ""
echo "Now serve the built app:"
echo "npx serve -s build -l 3000"
echo ""
echo "Or install serve globally: npm install -g serve" 