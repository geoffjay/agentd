# agentd-communicate API Reference

The communicate service provides REST and WebSocket APIs for managing rooms, participants, and messages. For a conceptual overview, see [Inter-Agent Communication](../communication.md).

## Base URL

```
http://127.0.0.1:17010
```

Set `AGENTD_COMMUNICATE_SERVICE_URL` to override (see [Configuration](../configuration.md)).

---

## Health

### `GET /health`

Liveness check.

**Response `200 OK`:**
```json
{
  "status": "ok",
  "service": "agentd-communicate",
  "version": "0.8.0"
}
```

---

## Rooms

### `POST /rooms`

Create a new room.

**Request body:**
```json
{
  "name": "engineering",
  "topic": "Engineering team coordination",
  "description": "General channel for engineering agents and humans",
  "room_type": "group",
  "created_by": "alice"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | — | Unique room name |
| `topic` | string | No | null | Short label |
| `description` | string | No | null | Longer description |
| `room_type` | string | No | `"group"` | `"direct"`, `"group"`, or `"broadcast"` |
| `created_by` | string | Yes | — | Creator identifier (agent UUID or username) |

**Response `201 Created`:**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "engineering",
  "topic": "Engineering team coordination",
  "description": "General channel for engineering agents and humans",
  "room_type": "group",
  "created_by": "alice",
  "created_at": "2026-03-19T12:00:00Z",
  "updated_at": "2026-03-19T12:00:00Z"
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| `400 Bad Request` | Empty name |
| `409 Conflict` | A room with this name already exists |

---

### `GET /rooms`

List rooms with optional type filter and pagination.

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 50 | Results per page (max 200) |
| `offset` | integer | 0 | Pagination offset |
| `room_type` | string | — | Filter by type: `direct`, `group`, or `broadcast` |

**Response `200 OK`:**
```json
{
  "items": [ /* RoomResponse objects */ ],
  "total": 12,
  "limit": 50,
  "offset": 0
}
```

---

### `GET /rooms/{id}`

Get a room by UUID.

**Response `200 OK`:** `RoomResponse`

**Errors:** `404 Not Found`

---

### `PUT /rooms/{id}`

Update a room's topic and/or description. Name and type cannot be changed after creation.

**Request body:**
```json
{
  "topic": "Updated topic",
  "description": "Updated description"
}
```

Both fields are optional. Omit a field to leave it unchanged.

**Response `200 OK`:** Updated `RoomResponse`

**Errors:** `404 Not Found`

---

### `DELETE /rooms/{id}`

Delete a room. Cascades to all participants and messages.

**Response `204 No Content`**

**Errors:** `404 Not Found`

---

## Participants

### `GET /rooms/{id}/participants`

List participants in a room.

**Query parameters:** `limit` (default 50, max 200), `offset` (default 0)

**Response `200 OK`:**
```json
{
  "items": [
    {
      "id": "uuid",
      "room_id": "550e8400-...",
      "identifier": "agent-abc",
      "kind": "agent",
      "display_name": "Worker Agent",
      "role": "member",
      "joined_at": "2026-03-19T12:00:00Z"
    }
  ],
  "total": 3,
  "limit": 50,
  "offset": 0
}
```

**Errors:** `404 Not Found` (room does not exist)

---

### `POST /rooms/{id}/participants`

Add a participant to a room.

**Request body:**
```json
{
  "identifier": "agent-abc",
  "kind": "agent",
  "display_name": "Worker Agent",
  "role": "member"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `identifier` | string | Yes | — | Agent UUID or username |
| `kind` | string | Yes | — | `"agent"` or `"human"` |
| `display_name` | string | Yes | — | Display name |
| `role` | string | No | `"member"` | `"member"`, `"admin"`, or `"observer"` |

**Response `201 Created`:** `ParticipantResponse`

WebSocket subscribers of the room receive a `participant_event` with `event: "joined"`.

**Errors:**

| Status | Condition |
|--------|-----------|
| `400 Bad Request` | Empty identifier |
| `404 Not Found` | Room does not exist |
| `409 Conflict` | Identifier is already a participant in this room |

---

### `GET /rooms/{id}/participants/{identifier}`

Get a specific participant by identifier.

**Response `200 OK`:** `ParticipantResponse`

**Errors:** `404 Not Found`

---

### `PUT /rooms/{id}/participants/{identifier}`

Update a participant's role within the room.

**Request body:**
```json
{ "role": "admin" }
```

**Response `200 OK`:** Updated `ParticipantResponse`

**Errors:** `404 Not Found`

---

### `DELETE /rooms/{id}/participants/{identifier}`

Remove a participant from a room.

**Response `204 No Content`**

WebSocket subscribers receive a `participant_event` with `event: "left"`.

**Errors:** `404 Not Found`

---

### `GET /participants/{identifier}/rooms`

List all rooms a participant belongs to.

**Query parameters:** `limit` (default 50, max 200), `offset` (default 0)

**Response `200 OK`:**
```json
{
  "items": [ /* RoomResponse objects */ ],
  "total": 2,
  "limit": 50,
  "offset": 0
}
```

---

## Messages

### `POST /rooms/{id}/messages`

Send a message to a room.

**Request body:**
```json
{
  "sender_id": "agent-abc",
  "sender_name": "Worker Agent",
  "sender_kind": "agent",
  "content": "Task complete — PR #42 created.",
  "metadata": { "pr_url": "https://github.com/org/repo/pull/42" },
  "reply_to": null
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `sender_id` | string | Yes | — | Must match an existing participant's identifier |
| `sender_name` | string | Yes | — | Display name captured at send time |
| `sender_kind` | string | Yes | — | `"agent"` or `"human"` |
| `content` | string | Yes | — | Message text (must be non-empty) |
| `metadata` | object | No | `{}` | Arbitrary key/value pairs |
| `reply_to` | UUID | No | null | ID of a message in the same room |

**Response `201 Created`:** `MessageResponse`

WebSocket subscribers receive a `message` server event.

**Errors:**

| Status | Condition |
|--------|-----------|
| `400 Bad Request` | Empty content; or `reply_to` references a message not in this room |
| `403 Forbidden` | `sender_id` is not a participant in the room |
| `404 Not Found` | Room does not exist |

---

### `GET /rooms/{id}/messages`

List messages in a room with optional timestamp filters.

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 50 | Results per page (max 200) |
| `offset` | integer | 0 | Pagination offset |
| `before` | RFC 3339 | — | Only messages created strictly before this timestamp |
| `after` | RFC 3339 | — | Only messages created strictly after this timestamp |

Messages are returned in ascending creation order.

**Response `200 OK`:**
```json
{
  "items": [ /* MessageResponse objects */ ],
  "total": 47,
  "limit": 50,
  "offset": 0
}
```

**Errors:** `404 Not Found`

---

### `GET /rooms/{id}/messages/latest`

Get the N most recent messages, returned oldest-first.

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `count` | integer | 50 | Number of messages to return |

**Response `200 OK`:** Array of `MessageResponse`

---

### `GET /messages/{id}`

Get a single message by UUID.

**Response `200 OK`:** `MessageResponse`

**Errors:** `404 Not Found`

---

### `DELETE /messages/{id}`

Delete a message.

**Response `204 No Content`**

**Errors:** `404 Not Found`

---

## WebSocket

### `GET /ws`

Establish a real-time connection to receive room events.

**Query parameters:**

| Parameter | Required | Description |
|-----------|----------|-------------|
| `identifier` | Yes | Agent UUID or human username |
| `kind` | Yes | `agent` or `human` |
| `display_name` | Yes | Display name for this connection |

**Example:**
```
ws://localhost:17010/ws?identifier=alice&kind=human&display_name=Alice
```

After upgrading, send JSON text frames. See [WebSocket Protocol](../communication.md#websocket-protocol) for the full message reference.

---

## Error Responses

All error responses follow the project-standard `ApiError` shape:

```json
{
  "error": "not found"
}
```

| HTTP Status | `ApiError` variant | Meaning |
|-------------|-------------------|---------|
| `400` | `InvalidInput` | Validation failed (empty field, bad enum value, etc.) |
| `403` | `Forbidden` | Sender is not a participant in the room |
| `404` | `NotFound` | Requested resource does not exist |
| `409` | `Conflict` | Duplicate name or identifier |
| `500` | `Internal` | Unexpected server error |

---

## Data Models

### `RoomResponse`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "engineering",
  "topic": "Engineering team coordination",
  "description": null,
  "room_type": "group",
  "created_by": "alice",
  "created_at": "2026-03-19T12:00:00Z",
  "updated_at": "2026-03-19T12:00:00Z"
}
```

### `ParticipantResponse`

```json
{
  "id": "uuid",
  "room_id": "550e8400-...",
  "identifier": "agent-abc",
  "kind": "agent",
  "display_name": "Worker Agent",
  "role": "member",
  "joined_at": "2026-03-19T12:00:00Z"
}
```

### `MessageResponse`

```json
{
  "id": "uuid",
  "room_id": "550e8400-...",
  "sender_id": "agent-abc",
  "sender_name": "Worker Agent",
  "sender_kind": "agent",
  "content": "Task complete",
  "metadata": {},
  "reply_to": null,
  "status": "sent",
  "created_at": "2026-03-19T12:00:00Z"
}
```

### `PaginatedResponse<T>`

```json
{
  "items": [],
  "total": 0,
  "limit": 50,
  "offset": 0
}
```

### Enums

**`room_type`:** `"direct"` | `"group"` | `"broadcast"`

**`participant kind`:** `"agent"` | `"human"`

**`participant role`:** `"member"` | `"admin"` | `"observer"`

**`message status`:** `"sent"` | `"delivered"` | `"read"`

---

## Running the Service

```bash
cargo run -p agentd-communicate
```

The service starts on `http://127.0.0.1:17010` by default. Override with `AGENTD_PORT`.

Prometheus metrics are available at `/metrics`.
