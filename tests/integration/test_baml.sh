#!/bin/bash
# Complete BAML test sequence

set -e

echo "=== BAML Integration Test ==="
echo ""

# Step 1: Verify BAML definitions
echo "1. Checking BAML definitions..."
baml check --from ./baml_src
echo "   ✓ BAML definitions valid"
echo ""

# Step 2: Check if Ollama is running
echo "2. Checking Ollama..."
if curl -s http://localhost:11434/api/tags >/dev/null 2>&1; then
    echo "   ✓ Ollama is running"

    # Check if the model exists
    if curl -s http://localhost:11434/api/tags | grep -q "gpt-oss:120b-cloud"; then
        echo "   ✓ Model gpt-oss:120b-cloud is available"
    else
        echo "   ⚠ Warning: Model gpt-oss:120b-cloud not found"
        echo "   Available models:"
        curl -s http://localhost:11434/api/tags | grep -o '"name":"[^"]*"' | cut -d'"' -f4
        echo ""
        echo "   To pull the model: ollama pull gpt-oss:120b-cloud"
        echo ""
    fi
else
    echo "   ✗ Ollama is not running"
    echo "   Start it with: ollama serve"
    exit 1
fi
echo ""

# Step 3: Start BAML server
echo "3. Starting BAML server..."
echo "   Starting baml serve on port 2024 (will run in background)..."

# Kill any existing BAML server
# pkill -f "baml serve" 2>/dev/null || true
# sleep 1

# Start BAML server in background
# baml serve --from ./baml_src > /tmp/baml-server.log 2>&1 &
# BAML_PID=$!
BAML_PID=$(overmind status | grep baml | awk '{ print $2 }')
echo "   BAML server PID: $BAML_PID"

# Wait for server to start
echo "   Waiting for BAML server to start..."
for i in {1..30}; do
    if curl -s http://localhost:2024/health >/dev/null 2>&1; then
        echo "   ✓ BAML server is running"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "   ✗ BAML server failed to start"
        echo "   Check logs: tail /tmp/baml-server.log"
        kill $BAML_PID 2>/dev/null || true
        exit 1
    fi
    sleep 1
done
echo ""

# Step 4: Test with curl
echo "4. Testing BAML function via curl..."
RESPONSE=$(curl -s -X POST http://localhost:2024/CategorizeNotification \
    -H "Content-Type: application/json" \
    -d '{
        "title": "Test Notification",
        "message": "This is a test message",
        "source_context": "test script"
    }')

if echo "$RESPONSE" | grep -q "category"; then
    echo "   ✓ BAML function call successful"
    echo "   Response preview:"
    echo "$RESPONSE" | head -c 200
    echo "..."
else
    echo "   ✗ BAML function call failed"
    echo "   Response: $RESPONSE"
    kill $BAML_PID 2>/dev/null || true
    exit 1
fi
echo ""

# Step 5: Run Rust example
echo "5. Running Rust example..."
echo ""
cargo run -p baml --example categorize_notification

# Cleanup
echo ""
echo "=== Test Complete ==="
echo ""
echo "BAML server is still running (PID: $BAML_PID)"
echo "To stop it: kill $BAML_PID"
echo "Or: pkill -f 'baml serve'"
echo ""
echo "Server logs: tail -f /tmp/baml-server.log"
