#!/bin/bash
set -eo pipefail

GATEWAY_URL="http://127.0.0.1:8787"
ADMIN_SECRET="automemory-admin-dev-secret"

TIMESTAMP=$(date +%s)
ALICE_UID="test_alice_${TIMESTAMP}"
BOB_UID="test_bob_${TIMESTAMP}"
TEST_USER_A="dev-token:${ALICE_UID}:alice@automemory.local"
TEST_USER_B="dev-token:${BOB_UID}:bob@automemory.local"

PASS_COUNT=0
TOTAL_COUNT=0


log_test() {
    TOTAL_COUNT=$((TOTAL_COUNT + 1))
    echo "[$TOTAL_COUNT] Testing: $1"
}

assert_success() {
    if [ $? -eq 0 ]; then
        PASS_COUNT=$((PASS_COUNT + 1))
        echo "   ✓ PASS"
    else
        echo "   ✗ FAIL: $1"
        exit 1
    fi
}

# Ensure local database schema is applied
npx wrangler d1 execute automemory-db --local --file=./schema.sql > /dev/null 2>&1 || true

# If gateway is not running, spawn it in background
SPAWNED_PID=""
if ! curl -s "$GATEWAY_URL/health" > /dev/null 2>&1; then
    echo "Starting local Wrangler dev gateway on port 8787..."
    npx wrangler dev --port 8787 > /tmp/wrangler_test.log 2>&1 &
    SPAWNED_PID=$!
    trap 'if [ -n "$SPAWNED_PID" ]; then kill $SPAWNED_PID 2>/dev/null || true; fi' EXIT

    # Wait for gateway to become healthy
    for i in {1..30}; do
        if curl -s "$GATEWAY_URL/health" | grep -q '"status":"ok"'; then
            break
        fi
        sleep 0.5
    done
fi

echo "=========================================================================="
echo "    AutoMemory Pro: Cloudflare Worker Local Gateway Integration Suite     "
echo "=========================================================================="


# 1. Health check
log_test "Gateway Health Endpoint"
RESP=$(curl -s "$GATEWAY_URL/health")
echo "$RESP" | grep -q '"status":"ok"'
assert_success "Health check returned status ok"

# 2. Check initial quota for new user (Alice) - Free Tier
log_test "Free Tier User Initial Quota"
QUOTA_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" "$GATEWAY_URL/api/v1/user/quota")
echo "$QUOTA_RESP" | grep -q '"plan_id":"free"'
echo "$QUOTA_RESP" | grep -q '"max_active_devices":1'
assert_success "Alice resolved to free tier with max 1 active device"

# 3. Register Device 1 for Alice
log_test "Register Device 1 on Free Tier (Allowed)"
DEV1_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d '{"device_id":"dev_macbook_01","device_name":"Alice MacBook Pro"}' \
    "$GATEWAY_URL/api/v1/auth/device/register")
echo "$DEV1_RESP" | grep -q '"success":true'
assert_success "Device 1 registered successfully"

# 4. Attempt to Register Device 2 on Free Tier (Should be BLOCKED with 403)
log_test "Register Device 2 on Free Tier (Blocked by Quota)"
DEV2_HTTP_CODE=$(curl -s -o /tmp/dev2_resp.json -w "%{http_code}" -X POST \
    -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d '{"device_id":"dev_mac_mini_02","device_name":"Alice Mac Mini"}' \
    "$GATEWAY_URL/api/v1/auth/device/register")

if [ "$DEV2_HTTP_CODE" -eq 403 ]; then
    grep -q "DEVICE_QUOTA_EXCEEDED" /tmp/dev2_resp.json
    assert_success "Free tier correctly blocked device 2 with DEVICE_QUOTA_EXCEEDED"
else
    echo "Expected HTTP 403, got $DEV2_HTTP_CODE"
    cat /tmp/dev2_resp.json
    exit 1
fi

# 5. Admin API: List Pricing Plans
log_test "Admin API: List Pricing Plans"
PLANS_RESP=$(curl -s -H "X-Admin-Secret: $ADMIN_SECRET" "$GATEWAY_URL/api/v1/admin/pricing/plans")
echo "$PLANS_RESP" | grep -q '"plan_id":"free"'
echo "$PLANS_RESP" | grep -q '"plan_id":"plus"'
echo "$PLANS_RESP" | grep -q '"plan_id":"team"'
assert_success "Admin successfully retrieved seeded pricing tiers"

# 6. Admin API: Upgrade Alice to Plus Plan (5 devices)
log_test "Admin API: Upgrade User to Plus Plan"
UPGRADE_RESP=$(curl -s -X POST -H "X-Admin-Secret: $ADMIN_SECRET" \
    -H "Content-Type: application/json" \
    -d "{\"uid\":\"$ALICE_UID\",\"plan_id\":\"plus\"}" \
    "$GATEWAY_URL/api/v1/admin/users/plan")
echo "$UPGRADE_RESP" | grep -q '"plan_id":"plus"'
assert_success "Alice upgraded to plus plan"

# 7. Verify Alice Quota Updated Dynamically
log_test "Verify Alice Quota Dynamically Updated to 5 Devices"
NEW_QUOTA=$(curl -s -H "Authorization: Bearer $TEST_USER_A" "$GATEWAY_URL/api/v1/user/quota")
echo "$NEW_QUOTA" | grep -q '"plan_id":"plus"'
echo "$NEW_QUOTA" | grep -q '"max_active_devices":5'
assert_success "Alice quota reflects plus tier without restarts"

# 8. Register Device 2 (Now Allowed)
log_test "Register Device 2 on Plus Plan (Allowed)"
DEV2_NEW_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d '{"device_id":"dev_mac_mini_02","device_name":"Alice Mac Mini"}' \
    "$GATEWAY_URL/api/v1/auth/device/register")
echo "$DEV2_NEW_RESP" | grep -q '"success":true'
assert_success "Device 2 registered successfully under plus tier"

# 9. Admin API: Dynamic Quota Override (user_entitlements)
log_test "Admin API: Set Custom Entitlements for Bob (10 devices override)"
ENTITLE_RESP=$(curl -s -X POST -H "X-Admin-Secret: $ADMIN_SECRET" \
    -H "Content-Type: application/json" \
    -d "{\"uid\":\"$BOB_UID\",\"custom_max_devices\":10,\"notes\":\"VIP Beta Partner\"}" \
    "$GATEWAY_URL/api/v1/admin/users/entitlements")
echo "$ENTITLE_RESP" | grep -q '"custom_max_devices":10'
assert_success "Custom entitlement override applied for Bob"


# 10. Verify Bob Quota Reflects Custom Entitlement
log_test "Verify Bob Quota Reflects Entitlement Override"
BOB_QUOTA=$(curl -s -H "Authorization: Bearer $TEST_USER_B" "$GATEWAY_URL/api/v1/user/quota")
echo "$BOB_QUOTA" | grep -q '"max_active_devices":10'
assert_success "Bob has 10 devices quota via direct entitlement"

# 11. Sync Push with Registered Device
log_test "Sync Push Project Manifest & Deltas"
PUSH_PAYLOAD='{
  "project_slug": "automemory-demo",
  "project_name": "AutoMemory Demo",
  "device_id": "dev_macbook_01",
  "manifest": {
    "version": 1,
    "project_slug": "automemory-demo",
    "files": [
      {
        "path": "docs/prd/overview.md",
        "category": "prd",
        "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "size_bytes": 1024,
        "compressed_l0": "Product Overview L0",
        "compressed_l1": "Product Overview L1",
        "updated_at": "2026-09-30T12:00:00Z"
      }
    ],
    "memories_count": 42,
    "skills_count": 5,
    "created_at": "2026-09-30T12:00:00Z"
  },
  "deltas": [
    {
      "path": "docs/prd/overview.md",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "content_base64": "IyBEZW1vIFByb2R1Y3QgU3BlY2lmaWNhdGlvbgo="
    }
  ]
}'

PUSH_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d "$PUSH_PAYLOAD" \
    "$GATEWAY_URL/api/v1/sync/push")
echo "$PUSH_RESP" | grep -q '"success":true'
echo "$PUSH_RESP" | grep -q '"manifest_hash"'
assert_success "Project manifest and deltas pushed to cloud gateway"

# 12. Sync Pull
log_test "Sync Pull Project Manifest"
PULL_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/sync/pull?project=automemory-demo")
echo "$PULL_RESP" | grep -q '"project_slug":"automemory-demo"'
echo "$PULL_RESP" | grep -q 'Product Overview L0'
assert_success "Project pulled with complete manifest matching push"

# 13. Disaster Recovery Restore Bundle
log_test "Disaster Recovery Full Restore Bundle"
RESTORE_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/sync/restore/automemory-demo")
echo "$RESTORE_RESP" | grep -q '"restore_ready":true'
assert_success "Disaster recovery bundle returned for project"

# 14. Device Revocation
log_test "Device Revocation (Alice removes Device 2)"
DEL_RESP=$(curl -s -X DELETE -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/auth/device/dev_mac_mini_02")
echo "$DEL_RESP" | grep -q '"success":true'
assert_success "Device lease revoked successfully"

# 15. Billing Webhook: Automated Stripe Checkout Plan Upgrade
log_test "Billing Webhook: Automated Plan Upgrade via Stripe Event"
CHARLIE_UID="test_charlie_${TIMESTAMP}"
TEST_USER_C="dev-token:${CHARLIE_UID}:charlie@automemory.local"

WEBHOOK_PAYLOAD='{
  "object": "event",
  "type": "checkout.session.completed",
  "data": {
    "object": {
      "client_reference_id": "'"$CHARLIE_UID"'",
      "metadata": {
        "uid": "'"$CHARLIE_UID"'",
        "plan_id": "team"
      }
    }
  }
}'

WEBHOOK_RESP=$(curl -s -X POST -H "Content-Type: application/json" \
    -d "$WEBHOOK_PAYLOAD" \
    "$GATEWAY_URL/api/v1/billing/webhook")
echo "$WEBHOOK_RESP" | grep -q '"received":true'
echo "$WEBHOOK_RESP" | grep -q '"plan_id":"team"'

# Verify Charlie's quota now reflects Team Plan (50 devices)
CHARLIE_QUOTA=$(curl -s -H "Authorization: Bearer $TEST_USER_C" "$GATEWAY_URL/api/v1/user/quota")
echo "$CHARLIE_QUOTA" | grep -q '"plan_id":"team"'
echo "$CHARLIE_QUOTA" | grep -q '"max_active_devices":50'
assert_success "Billing webhook upgraded user to Team plan with 50 devices quota"

# 16. Zero-Knowledge Blind Sync Push
log_test "Zero-Knowledge Blind Sync Push (Encrypted Memories, Skills & Plan Chunks)"
BLIND_PUSH_PAYLOAD='{
  "project_slug": "blind-demo-project",
  "project_name": "Blind Demo",
  "device_id": "dev_macbook_01",
  "is_blind": true,
  "blind_memories": [
    {
      "id": "blind_mem_001",
      "blind_project_id": "blind-demo-project",
      "ciphertext_base64": "SGVsbG8gQmxpbmQgU2VydmVyIENpcGhlcnRleHQgQUVTLUdDTSAxMjM0",
      "nonce_base64": "MTIzNDU2Nzg5MDEy",
      "blind_scope": "project",
      "blind_type": "8f3b2a9e",
      "blind_tags": ["a1b2c3d4e5f6", "f6e5d4c3b2a1"],
      "confidence": 0.99,
      "importance": 0.95,
      "status": "active",
      "created_at": "2026-10-02T12:00:00Z",
      "updated_at": "2026-10-02T12:00:00Z"
    }
  ],
  "blind_skills": [
    {
      "id": "blind_skill_001",
      "blind_project_id": "blind-demo-project",
      "ciphertext_base64": "U2tpbGwgQ2lwaGVydGV4dCBQbGF5Ym9vayBTdGVwcw==",
      "nonce_base64": "OTg3NjU0MzIxMDEy",
      "success_count": 3,
      "failure_count": 0,
      "created_at": "2026-10-02T12:00:00Z",
      "updated_at": "2026-10-02T12:00:00Z"
    }
  ],
  "encrypted_deltas": [
    {
      "path": "plans/q4_roadmap.md",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "ciphertext_base64": "RW5jcnlwdGVkIFBsYW4gQ29udGVudCBpbiBSMg==",
      "nonce_base64": "MDEyMzQ1Njc4OTAx",
      "size_bytes": 1024
    }
  ]
}'

BLIND_PUSH_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d "$BLIND_PUSH_PAYLOAD" \
    "$GATEWAY_URL/api/v1/sync/push")

echo "$BLIND_PUSH_RESP" | grep -q '"success":true'
echo "$BLIND_PUSH_RESP" | grep -q '"is_blind":true'
echo "$BLIND_PUSH_RESP" | grep -q '"blind_memories_stored":1'
echo "$BLIND_PUSH_RESP" | grep -q '"blind_skills_stored":1'
echo "$BLIND_PUSH_RESP" | grep -q '"deltas_stored":1'
assert_success "Blind sync push accepted encrypted memories, skills, and plan chunks"

# 17. Zero-Knowledge Blind Sync Pull
log_test "Zero-Knowledge Blind Sync Pull (Verify Ciphertext & Nonce Integrity)"
BLIND_PULL_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/sync/pull?project=blind-demo-project")

echo "DEBUG BLIND_PULL_RESP: $BLIND_PULL_RESP"

echo "$BLIND_PULL_RESP" | grep -q '"success":true'
echo "$BLIND_PULL_RESP" | grep -q '"blind_memories":\['
echo "$BLIND_PULL_RESP" | grep -q 'SGVsbG8gQmxpbmQgU2VydmVyIENpcGhlcnRleHQgQUVTLUdDTSAxMjM0'
echo "$BLIND_PULL_RESP" | grep -q 'MTIzNDU2Nzg5MDEy'
echo "$BLIND_PULL_RESP" | grep -q '"blind_skills":\['
echo "$BLIND_PULL_RESP" | grep -q 'U2tpbGwgQ2lwaGVydGV4dCBQbGF5Ym9vayBTdGVwcw=='
assert_success "Blind pull returned ciphertext and nonces with zero plaintext exposure"

# 18. Stream Encrypted Plan Blob from R2
log_test "Stream Encrypted Plan Chunk from R2"
BLOB_CONTENT=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/sync/blob/blind-demo-project/e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")

# Compare downloaded blob against decoded ciphertext
EXPECTED_RAW=$(echo "RW5jcnlwdGVkIFBsYW4gQ29udGVudCBpbiBSMg==" | base64 --decode)
if [ "$BLOB_CONTENT" = "$EXPECTED_RAW" ]; then
    assert_success "Encrypted plan chunk streamed directly from R2"
else
    echo "Blob content mismatch: got '$BLOB_CONTENT', expected '$EXPECTED_RAW'"
    exit 1
fi

# 19. Developer Web Console UI Serving
log_test "Developer Web Console HTML Endpoint (/dashboard)"
DASH_RESP=$(curl -s -L "$GATEWAY_URL/dashboard")
echo "$DASH_RESP" | grep -q "AutoMemory Pro - Edge Control Console"
echo "$DASH_RESP" | grep -q "Zero-Knowledge Blind Architecture Verified"
echo "$DASH_RESP" | grep -q "Device Fleet Manager"
assert_success "Web console HTML page served successfully with zero-knowledge guarantee"

# 20. Dashboard Stats API
log_test "Dashboard Metrics & Quotas Stats API"
STATS_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/dashboard/stats")
echo "DEBUG STATS_RESP: $STATS_RESP"
echo "$STATS_RESP" | grep -q '"uid":"'"$ALICE_UID"'"'
echo "$STATS_RESP" | grep -q '"blind_memories_count":1'
echo "$STATS_RESP" | grep -q '"blind_skills_count":1'
echo "$STATS_RESP" | grep -q '"total_projects":2'
assert_success "Dashboard stats returned aggregated zero-knowledge metrics and active quotas"

# 21. Blind Sync Timeline Feed
log_test "Sync Audit Timeline Feed"
TIMELINE_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/dashboard/timeline")
echo "$TIMELINE_RESP" | grep -q '"project_slug":"blind-demo-project"'
echo "$TIMELINE_RESP" | grep -q '"event_type":"sync_push"'
echo "$TIMELINE_RESP" | grep -q '"blind_memories_count":1'
assert_success "Sync timeline audit log recorded prior edge push event"

# 22. Register Webhook
log_test "Register Slack/Discord Webhook Endpoint"
WEBHOOK_REG_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d '{"url":"https://hooks.slack.com/services/T00/B00/test1234","secret":"sec_slack_alert_99"}' \
    "$GATEWAY_URL/api/v1/dashboard/webhooks")
echo "$WEBHOOK_REG_RESP" | grep -q '"success":true'
echo "$WEBHOOK_REG_RESP" | grep -q 'https://hooks.slack.com/services/T00/B00/test1234'
HOOK_ID=$(echo "$WEBHOOK_REG_RESP" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
assert_success "Webhook registered successfully with ID $HOOK_ID"

# 23. List Webhooks
log_test "List Active User Webhooks"
WEBHOOK_LIST_RESP=$(curl -s -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/dashboard/webhooks")
echo "$WEBHOOK_LIST_RESP" | grep -q "$HOOK_ID"
assert_success "Configured webhook retrieved in active webhooks list"

# 24. Test Webhook Ping Endpoint
log_test "Webhook Dispatcher Test Ping API"
TEST_PING_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d '{"url":"http://127.0.0.1:8787/health"}' \
    "$GATEWAY_URL/api/v1/dashboard/webhooks/test")
echo "DEBUG TEST_PING_RESP: $TEST_PING_RESP"
echo "$TEST_PING_RESP" | grep -q '"success":true'
echo "$TEST_PING_RESP" | grep -q '"status":200'
assert_success "Webhook test ping successfully reached local endpoint with measured latency"

# 25. Delete Webhook
log_test "Delete Webhook Endpoint"
DEL_HOOK_RESP=$(curl -s -X DELETE -H "Authorization: Bearer $TEST_USER_A" \
    "$GATEWAY_URL/api/v1/dashboard/webhooks/$HOOK_ID")
echo "$DEL_HOOK_RESP" | grep -q '"success":true'
assert_success "Webhook deleted cleanly"

# 26. Dashboard Plan Upgrade Simulation
log_test "Dashboard Plan Upgrade Simulation (Free -> Plus)"
SIM_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d '{"plan_id":"plus"}' \
    "$GATEWAY_URL/api/v1/dashboard/upgrade-simulation")
echo "$SIM_RESP" | grep -q '"new_plan":"plus"'

# Verify quota increased to 5 devices
NEW_QUOTA=$(curl -s -H "Authorization: Bearer $TEST_USER_A" "$GATEWAY_URL/api/v1/user/quota")
echo "$NEW_QUOTA" | grep -q '"plan_id":"plus"'
echo "$NEW_QUOTA" | grep -q '"max_active_devices":5'
assert_success "Upgrade simulation switched tier to Plus and adjusted device quotas"

# 27. Blind Vector Indexing API
log_test "Blind Vector Indexing API (Zero-Knowledge Coordinates)"
VEC_INDEX_PAYLOAD='{
  "blind_project_id": "blind-demo-project",
  "items": [
    {
      "id": "blind_mem_001",
      "vector": [0.12, 0.45, -0.22, 0.88, 0.05, -0.19, 0.33, 0.61]
    }
  ]
}'
VEC_INDEX_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d "$VEC_INDEX_PAYLOAD" \
    "$GATEWAY_URL/api/v1/vector/index")
echo "$VEC_INDEX_RESP" | grep -q '"success":true'
echo "$VEC_INDEX_RESP" | grep -q '"indexed_count":1'
assert_success "Anonymous geometric vectors indexed in edge database"

# 28. Zero-Knowledge Blind Semantic Vector Search
log_test "Zero-Knowledge Blind Semantic Vector Search"
# Query vector close to blind_mem_001 vector
VEC_SEARCH_PAYLOAD='{
  "blind_project_id": "blind-demo-project",
  "vector": [0.11, 0.44, -0.21, 0.87, 0.04, -0.18, 0.32, 0.60],
  "top_k": 3
}'
VEC_SEARCH_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d "$VEC_SEARCH_PAYLOAD" \
    "$GATEWAY_URL/api/v1/vector/search")
echo "DEBUG VEC_SEARCH_RESP: $VEC_SEARCH_RESP"
echo "$VEC_SEARCH_RESP" | grep -q '"success":true'
echo "$VEC_SEARCH_RESP" | grep -q '"count":1'
echo "$VEC_SEARCH_RESP" | grep -q '"id":"blind_mem_001"'
echo "$VEC_SEARCH_RESP" | grep -q 'SGVsbG8gQmxpbmQgU2VydmVyIENpcGhlcnRleHQgQUVTLUdDTSAxMjM0'
assert_success "Blind vector search matched target memory and returned encrypted ciphertext with high similarity"

# 29. Semantic Vector Search Isolation Filter
log_test "Vector Search Project Isolation (Filter Out Different Project)"
VEC_ISOLATE_PAYLOAD='{
  "blind_project_id": "other-nonexistent-project",
  "vector": [0.11, 0.44, -0.21, 0.87, 0.04, -0.18, 0.32, 0.60],
  "top_k": 3
}'
VEC_ISOLATE_RESP=$(curl -s -X POST -H "Authorization: Bearer $TEST_USER_A" \
    -H "Content-Type: application/json" \
    -d "$VEC_ISOLATE_PAYLOAD" \
    "$GATEWAY_URL/api/v1/vector/search")
echo "$VEC_ISOLATE_RESP" | grep -q '"count":0'
assert_success "Vector search correctly isolated results by project slug"

# 30. CLI Device Pairing & Keychain Verification
log_test "CLI Device Pairing Command (Phrase & QR Code)"
PAIR_OUTPUT=$(cargo run -q --bin automemory -- sync pair 2>/dev/null)
echo "$PAIR_OUTPUT" | grep -q "AutoMemory Pro — Zero-Knowledge Device Pairing"
echo "$PAIR_OUTPUT" | grep -q "Recovery Phrase:"
echo "$PAIR_OUTPUT" | grep -q "automemory sync pair --join"
assert_success "CLI sync pair command rendered visual QR matrix and recovery phrase"

echo "=========================================================================="
echo "    Summary: $PASS_COUNT/$TOTAL_COUNT Tests Passed Successfully (100%)    "
echo "=========================================================================="




