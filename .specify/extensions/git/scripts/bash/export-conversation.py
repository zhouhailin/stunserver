#!/usr/bin/env python3
"""
Export conversation history from JSONL to Markdown format.

Usage: export-conversation.py <session_id> <output_dir>

Reads the conversation from ~/.qoder-cn/projects/<project-path>/<session_id>.jsonl
and exports it to <output_dir>/conversation-<session_id>.md
"""

import json
import os
import sys
from datetime import datetime
from pathlib import Path


def get_project_path():
    """Get the current project path identifier."""
    cwd = os.getcwd()
    # Convert /root/ai-coding-template to -root-ai-coding-template
    return cwd.replace("/", "-")


def find_conversation_file(session_id):
    """Find the JSONL file for the given session ID."""
    project_path = get_project_path()
    qoder_dir = Path.home() / ".qoder-cn" / "projects" / project_path
    jsonl_file = qoder_dir / f"{session_id}.jsonl"

    if jsonl_file.exists():
        return jsonl_file

    # Try to find the most recent file if exact match not found
    if qoder_dir.exists():
        jsonl_files = list(qoder_dir.glob("*.jsonl"))
        if jsonl_files:
            # Return the most recently modified file
            return max(jsonl_files, key=lambda f: f.stat().st_mtime)

    return None


def get_session_creation_time(jsonl_file):
    """Extract session creation time from the first record in JSONL file."""
    with open(jsonl_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
                timestamp = entry.get("timestamp", "")
                if timestamp:
                    # Handle both ISO format strings and millisecond timestamps
                    if isinstance(timestamp, int):
                        dt = datetime.fromtimestamp(timestamp / 1000)
                    else:
                        dt = datetime.fromisoformat(str(timestamp).replace("Z", "+00:00"))
                    return dt.strftime("%Y%m%d-%H%M%S")
            except (json.JSONDecodeError, ValueError):
                continue
    return datetime.now().strftime("%Y%m%d-%H%M%S")


def parse_conversation(jsonl_file):
    """Parse JSONL file and extract user/assistant messages."""
    messages = []

    with open(jsonl_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue

            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue

            msg_type = entry.get("type")
            if msg_type not in ["user", "assistant"]:
                continue

            # Skip meta messages (system commands, etc.)
            if entry.get("isMeta"):
                continue

            timestamp = entry.get("timestamp", "")
            message = entry.get("message", {})
            content = message.get("content", "")

            # Handle different content formats
            if isinstance(content, list):
                # Assistant messages often have content as a list of blocks
                text_parts = []
                for block in content:
                    if block.get("type") == "text":
                        text_parts.append(block.get("text", ""))
                    elif block.get("type") == "thinking":
                        # Skip thinking blocks for cleaner output
                        continue
                content = "\n\n".join(text_parts)

            if content:
                messages.append(
                    {
                        "type": msg_type,
                        "timestamp": timestamp,
                        "content": content,
                    }
                )

    return messages


def format_markdown(session_id, messages):
    """Format messages as Markdown."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    lines = [
        "# 会话记录",
        "",
        f"**会话 ID**: {session_id}",
        f"**导出时间**: {now}",
        "",
        "---",
        "",
    ]

    for msg in messages:
        role = "用户" if msg["type"] == "user" else "助手"
        timestamp = msg["timestamp"]

        # Format timestamp if present
        if timestamp:
            try:
                dt = datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
                formatted_time = dt.strftime("%Y-%m-%d %H:%M:%S")
            except:
                formatted_time = timestamp
        else:
            formatted_time = ""

        lines.append(f"## {role} ({formatted_time})")
        lines.append("")
        lines.append(msg["content"])
        lines.append("")
        lines.append("---")
        lines.append("")

    return "\n".join(lines)


def main():
    if len(sys.argv) < 3:
        print("Usage: export-conversation.py <session_id> <output_dir>")
        sys.exit(1)

    session_id = sys.argv[1]
    output_dir = sys.argv[2]

    # Find conversation file
    jsonl_file = find_conversation_file(session_id)
    if not jsonl_file:
        print(f"Warning: No conversation file found for session {session_id}")
        sys.exit(0)

    # Parse conversation
    messages = parse_conversation(jsonl_file)
    if not messages:
        print(f"Warning: No messages found in conversation {session_id}")
        sys.exit(0)

    # Get session creation time for filename
    creation_time = get_session_creation_time(jsonl_file)

    # Format as Markdown
    markdown_content = format_markdown(session_id, messages)

    # Write to output file with creation time in filename
    output_file = Path(output_dir) / f"conversation-{creation_time}-{session_id}.md"
    output_file.parent.mkdir(parents=True, exist_ok=True)

    with open(output_file, "w", encoding="utf-8") as f:
        f.write(markdown_content)

    print(f"[OK] Conversation exported to {output_file}")


if __name__ == "__main__":
    main()
