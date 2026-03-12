#!/bin/bash

input=$(cat)

# Format the message: extract tool_name and tool_input, create a more readable structure
formatted=$(echo "$input" | jq -c '{tool: .tool_name, input: .tool_input}')

AGENTD_NOTIFY_SERVICE_URL=http://localhost:7004 agent notify create -r -t agentd -l ephemeral -m "$formatted"
