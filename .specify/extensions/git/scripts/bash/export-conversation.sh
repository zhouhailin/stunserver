#!/usr/bin/env bash
# Export conversation history from JSONL to Markdown format.
#
# Usage: export-conversation.sh <session_id> <output_dir>
#
# Reads the conversation from ~/.qoder-cn/projects/<project-path>/<session_id>.jsonl
# and exports it to <output_dir>/conversation-<creation-time>-<session_id>.md

set -e

SESSION_ID="${1:-}"
OUTPUT_DIR="${2:-}"

if [ -z "$SESSION_ID" ] || [ -z "$OUTPUT_DIR" ]; then
    echo "Usage: $0 <session_id> <output_dir>" >&2
    exit 1
fi

# Check if jq is available
if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required but not installed" >&2
    exit 1
fi

# Get current project path identifier
get_project_path() {
    local cwd
    cwd="$(pwd)"
    # Convert /root/ai-coding-template to -root-ai-coding-template
    echo "$cwd" | sed 's|/|-|g'
}

# Find the JSONL file for the given session ID
find_conversation_file() {
    local session_id="$1"
    local project_path
    project_path="$(get_project_path)"
    local qoder_dir="$HOME/.qoder-cn/projects/$project_path"
    local jsonl_file="$qoder_dir/$session_id.jsonl"

    if [ -f "$jsonl_file" ]; then
        echo "$jsonl_file"
        return 0
    fi

    # Try to find the most recent file if exact match not found
    if [ -d "$qoder_dir" ]; then
        local most_recent
        most_recent=$(find "$qoder_dir" -maxdepth 1 -name "*.jsonl" -type f -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-)
        if [ -n "$most_recent" ]; then
            echo "$most_recent"
            return 0
        fi
    fi

    return 1
}

# Extract session creation time from the first record
get_session_creation_time() {
    local jsonl_file="$1"
    local timestamp

    # Read first line and extract timestamp
    timestamp=$(head -1 "$jsonl_file" | jq -r '.timestamp // empty' 2>/dev/null)

    if [ -z "$timestamp" ]; then
        date +"%Y%m%d-%H%M%S"
        return
    fi

    # Handle both ISO format strings and millisecond timestamps
    if [[ "$timestamp" =~ ^[0-9]+$ ]]; then
        # Millisecond timestamp
        date -d "@$((timestamp / 1000))" +"%Y%m%d-%H%M%S" 2>/dev/null || date +"%Y%m%d-%H%M%S"
    else
        # ISO format string
        date -d "$timestamp" +"%Y%m%d-%H%M%S" 2>/dev/null || date +"%Y%m%d-%H%M%S"
    fi
}

# Format timestamp for display
format_timestamp() {
    local timestamp="$1"
    if [ -z "$timestamp" ]; then
        echo ""
        return
    fi

    if [[ "$timestamp" =~ ^[0-9]+$ ]]; then
        date -d "@$((timestamp / 1000))" +"%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "$timestamp"
    else
        date -d "$timestamp" +"%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "$timestamp"
    fi
}

# Find conversation file
jsonl_file=$(find_conversation_file "$SESSION_ID")
if [ -z "$jsonl_file" ] || [ ! -f "$jsonl_file" ]; then
    echo "Warning: No conversation file found for session $SESSION_ID" >&2
    exit 0
fi

# Get session creation time for filename
creation_time=$(get_session_creation_time "$jsonl_file")

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Output file path
output_file="$OUTPUT_DIR/conversation-$creation_time-$SESSION_ID.md"

# Start writing markdown
{
    echo "# 会话记录"
    echo ""
    echo "**会话 ID**: $SESSION_ID"
    echo "**导出时间**: $(date +"%Y-%m-%d %H:%M:%S")"
    echo ""
    echo "---"
    echo ""
} > "$output_file"

# Parse JSONL and extract user/assistant messages
# Filter: type is "user" or "assistant", isMeta is false or not present, content is not empty
jq -c 'select(.type == "user" or .type == "assistant") | select(.isMeta != true) | select(.message.content != null and .message.content != "")' "$jsonl_file" 2>/dev/null | while IFS= read -r entry; do
    msg_type=$(echo "$entry" | jq -r '.type')
    timestamp=$(echo "$entry" | jq -r '.timestamp // ""')
    content=$(echo "$entry" | jq -r '.message.content')

    # Handle content as array (assistant messages)
    if echo "$content" | jq -e 'type == "array"' >/dev/null 2>&1; then
        # Extract text blocks from array
        content=$(echo "$content" | jq -r 'map(select(.type == "text") | .text) | join("\n\n")')
    fi

    # Skip if content is empty after processing
    if [ -z "$content" ] || [ "$content" = "null" ]; then
        continue
    fi

    # Format role
    if [ "$msg_type" = "user" ]; then
        role="用户"
    else
        role="助手"
    fi

    # Format timestamp
    formatted_time=$(format_timestamp "$timestamp")

    # Write to markdown
    {
        echo "## $role ($formatted_time)"
        echo ""
        echo "$content"
        echo ""
        echo "---"
        echo ""
    } >> "$output_file"
done

echo "[OK] Conversation exported to $output_file"
