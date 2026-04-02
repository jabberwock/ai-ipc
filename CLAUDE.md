# Claude IPC - Collaboration Tool

## Overview

`collab` is a command-line tool that enables multiple Claude Code instances to communicate and collaborate with each other. It allows workers to send messages, reference previous communications, and stay synchronized on fixes and updates.

## Installation

### Prerequisites

- **Rust toolchain** (1.70+): Install from [rustup.rs](https://rustup.rs)
- **SQLite3**: Pre-installed on most systems

### Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd claude-ipc

# Build the server
cd collab-server
cargo build --release

# Build the CLI
cd ../collab-cli
cargo build --release
```

### Install Binaries

```bash
# Server binary
cp collab-server/target/release/collab-server /usr/local/bin/

# CLI binary
cp collab-cli/target/release/collab /usr/local/bin/
```

## Watch the Demo

[TODO: YouTube demo video showing full workflow]

## Quick Start (5 minutes)

### Step 1: Initialize Your Workers

Create a `workers.yaml` file in your project:

```yaml
server: http://localhost:8000
workers:
  - name: frontend
    role: "Frontend development"
  - name: backend
    role: "Backend API development"
```

Run initialization:

```bash
collab init workers.yaml
```

### Step 2: Start the Server and Workers

Open two terminals side-by-side. In the first, start the server (keep it running):

```bash
# Terminal 1 — keeps running
collab-server
```

In the second terminal, start the workers:

```bash
# Terminal 2
collab start all
```

Verify they're running:

```bash
collab lifecycle-status
```

Expected output:
```
Running workers:
  frontend (PID: 12345)
  backend (PID: 12346)
```

✓ If you see this, you're good to go to Step 3.

### Step 3: Send Your First Message

Open a third terminal and start watching for messages:

```bash
# Terminal 3
export COLLAB_INSTANCE=frontend
collab stream --role "Building login UI"
```

Now in a fourth terminal, send a message from the backend worker:

```bash
# Terminal 4
export COLLAB_INSTANCE=backend
collab add @frontend "Login endpoint ready at POST /auth/login"
```

Watch Terminal 3 — you'll see the message appear instantly. ✓

**That's it.** You now have two independent Claude workers collaborating in real-time. From here, you can:
- Run `collab stop all` to stop workers
- Run `collab start all` to start them again  
- Check the [Messaging Commands](#messaging-commands) section for more options

## Worker Management

These commands manage the lifecycle of worker processes:

### `collab init [FILE]`

Set up worker environments from a YAML config or interactive wizard.

```bash
# From a YAML file
collab init workers.yaml

# Interactive wizard (if 'monitor' feature is enabled)
collab init
```

**YAML Format:**
```yaml
server: http://localhost:8000
output_dir: ./workers     # optional — where to create worker directories
workers:
  - name: frontend
    role: "Build the React UI and manage component state"
  - name: backend
    role: "Implement REST API endpoints and database queries"
```

Creates a `.collab/workers.json` manifest and CLAUDE.md files in each worker directory.

### `collab start <TARGET>`

Start worker process(es) in the background.

```bash
# Start a specific worker
collab start @frontend

# Start all workers
collab start all
```

Workers run as background processes managed by the collab system. Each worker receives incoming messages via `collab stream` and can process them with configured Claude Code instances.

### `collab stop <TARGET>`

Stop running worker process(es).

```bash
# Stop a specific worker
collab stop @backend

# Stop all workers
collab stop all
```

### `collab restart <TARGET>`

Stop and restart worker process(es).

```bash
# Restart a specific worker
collab restart @frontend

# Restart all workers
collab restart all
```

### `collab lifecycle-status`

Show all running worker processes, their PIDs, and startup timestamps.

```bash
collab lifecycle-status
```

**Output:**
```
Running workers:
  frontend (PID: 12345)
    Started: 2024-03-27 14:25:10 UTC
    Command: collab worker --workdir ./workers/frontend --model haiku
  backend (PID: 12346)
    Started: 2024-03-27 14:25:11 UTC
    Command: collab worker --workdir ./workers/backend --model haiku
```

## Messaging Commands

These commands send and receive messages between workers:

### `collab list`

List all messages intended for your instance from the last hour.

```bash
collab --instance worker1 list
```

**Output:**
```
Messages for @worker1:

─────────────────────────────────────
Hash: a94a8fe5ccb19ba61c4c0873d391e987982fbbd3
From: @worker2
Time: 2024-03-27 14:30:45 UTC

Applied your authentication fix and tested successfully
─────────────────────────────────────
```

#### `collab add @<instance> <message>`

Send a message to another worker instance.

```bash
# Basic message
collab add @worker2 "Completed database migration"

# Message with references to previous messages
collab add @worker2 "Applied your suggestions" \
  --refs a94a8fe5ccb19ba61c4c0873d391e987982fbbd3
```

**Options:**
- `--refs <HASH1,HASH2>`: Reference previous message hashes (comma-separated)

#### `collab stream`

Stream incoming messages in real-time via Server-Sent Events (SSE). Zero-polling, instant delivery.

```bash
# Stream messages and set your role
collab stream --role "Building API endpoints"
```

**Output:**
```
🔌 Streaming messages for @backend
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔔 New message!
Hash: b3d5c3a...
From: @frontend
Time: 2024-03-27 14:35:12 UTC

Fixed the auth redirect issue
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## Configuration

Configuration can be provided via (in priority order, top of list wins):

1. **CLI flags**: `--instance`, `--server`
2. **Environment variables**: `$COLLAB_INSTANCE`, `$COLLAB_SERVER`, `$COLLAB_TOKEN`
3. **.env file**: `.env` in your project (auto-loaded from cwd or parent directories)
4. **Config file**: `.collab.toml` in your project or `~/.collab.toml` globally

**Example ~/.collab.toml:**
```toml
host = "http://localhost:8000"
instance = "frontend"
token = "optional-auth-token"
recipients = ["backend", "database"]
```

**Example .env file:**
```bash
COLLAB_INSTANCE=frontend
COLLAB_SERVER=http://localhost:8000
COLLAB_TOKEN=your-auth-token
```

### Global Options

- `--server <URL>`: Server URL (default: `http://localhost:8000`)
- `--instance <ID>`: Your worker instance identifier (required for most commands)
- `--help`: Show command help

## Message Format

Every message includes:

- **Hash**: SHA1 hash (40 hex characters) of the message content - use this to reference messages
- **Sender**: Instance ID of the sender (e.g., `worker1`)
- **Recipient**: Instance ID of the recipient (e.g., `worker2`)
- **Content**: The actual message text
- **Refs**: Array of referenced message hashes (for conversation threading)
- **Timestamp**: UTC timestamp in RFC3339 format

### Message Addressing

Messages are identified by their SHA1 hash, which allows workers to reference specific communications:

```bash
# Worker 1 sends a message
collab add @worker2 "Found bug in payment processor"
# Output: Hash: abc123...

# Worker 2 responds, referencing the original message
collab add @worker1 "Fixed the bug you mentioned" --refs abc123...
```

## Use Cases

### Scenario 1: Multi-Worker Setup

Initialize workers and start them all:

```bash
# 1. Create workers.yaml
cat > workers.yaml <<EOF
server: http://localhost:8000
workers:
  - name: frontend
    role: "React UI and component state"
  - name: backend
    role: "REST API and database queries"
EOF

# 2. Initialize and start
collab init workers.yaml
collab start all

# 3. Set up environment for frontend worker
export COLLAB_INSTANCE=frontend

# 4. Stream messages in a dedicated terminal
collab stream --role "Building authentication UI"
```

### Scenario 2: Bug Fix Coordination

**Frontend worker**:
```bash
export COLLAB_INSTANCE=frontend
collab add @backend "Authentication redirects to 404 after login"
```

**Backend worker** (receives notification via `collab stream`):
```bash
collab list
# Sees the message, fixes the route

collab add @frontend "Fixed route in auth.rs - commit a7b3c2" \
  --refs <hash-from-frontend-message>
```

**Frontend worker** (sees notification in streaming session):
```bash
# Message appears automatically in the collab stream terminal
# Pulls changes and tests
```

### Scenario 3: Continuous Collaboration with Background Workers

```bash
# Terminal 1: Start the server
collab-server

# Terminal 2: Start all workers
collab start all

# Terminal 3+: Stream messages for each worker
export COLLAB_INSTANCE=frontend
collab stream --role "Building API integration"

# Terminal 4+: Work on code, send messages
export COLLAB_INSTANCE=backend
collab add @frontend "API endpoints ready for integration"

# Receive notifications in the frontend stream terminal in real-time
```

## Message Retention

- Messages are retained for **1 hour** from creation time
- After 1 hour, messages are filtered out from `list` and `stream` results
- Use the server's `/messages/cleanup` endpoint to manually purge old messages

```bash
curl -X DELETE http://localhost:8000/messages/cleanup
```

## Server API

The server exposes a REST API for integration:

### `GET /messages/{instance_id}`

Retrieve messages for a specific instance (last hour only).

**Response:**
```json
[
  {
    "id": "uuid",
    "hash": "sha1-hash-of-content",
    "sender": "worker1",
    "recipient": "worker2",
    "content": "Message text",
    "refs": ["hash1", "hash2"],
    "timestamp": "2024-03-27T14:30:45Z"
  }
]
```

### `POST /messages`

Create a new message.

**Request:**
```json
{
  "sender": "worker1",
  "recipient": "worker2",
  "content": "Message text",
  "refs": ["hash1", "hash2"]
}
```

### `DELETE /messages/cleanup`

Remove messages older than 1 hour.

## Best Practices

### 1. Use Descriptive Instance IDs

Choose instance IDs that reflect the worker's role:
```bash
export COLLAB_INSTANCE=frontend-worker
export COLLAB_INSTANCE=api-worker
export COLLAB_INSTANCE=database-worker
```

### 2. Use `stream` for Active Collaboration

During intensive collaboration sessions, run `collab stream` in a dedicated terminal to get real-time notifications.

### 3. Reference Previous Messages

Always use `--refs` when responding to maintain conversation context:
```bash
collab add @other "Applied your fix" --refs abc123def456
```

### 4. Keep Messages Concise but Informative

Good:
```bash
collab add @api "Added user.avatar_url field - migration pending"
```

Bad:
```bash
collab add @api "did some stuff"
```

### 5. Include Relevant Context

Mention files, commits, or specific changes:
```bash
collab add @frontend "Updated API endpoint in users.ts:45 - now returns 201 instead of 200"
```

## Troubleshooting

### Instance ID Required

**Problem:** `Instance ID required. Set via --instance, $COLLAB_INSTANCE, or ~/.collab.toml`

**Solution:** Set your instance ID before running commands:
```bash
# Option 1: Environment variable
export COLLAB_INSTANCE=frontend

# Option 2: CLI flag
collab --instance frontend list

# Option 3: Config file (~/.collab.toml)
echo 'instance = "frontend"' >> ~/.collab.toml
```

### Connection Refused

**Problem:** `Connection error: connection refused`

**Solution:** Ensure the server is running in a separate terminal:
```bash
collab-server
```

You should see: `Server listening on http://127.0.0.1:8000`

### Workers Not Starting

**Problem:** `collab start all` fails or workers don't appear in `collab lifecycle-status`

**Solution:**
1. Verify you ran `collab init` in your project directory
2. Check that `.collab/workers.json` exists:
   ```bash
   cat .collab/workers.json
   ```
3. Verify your instance IDs match the worker names defined in `workers.yaml`

### No Messages Appearing

**Problem:** Running `collab list` shows no messages.

**Solution:**
1. Verify you're using the correct instance ID: `echo $COLLAB_INSTANCE`
2. Check that messages were sent in the last hour (messages older than 1 hour are auto-purged)
3. Confirm messages were addressed TO your instance ID
4. Verify the server is running and reachable

### Server Not Starting

**Problem:** Server fails to bind to port 8000.

**Solution:** Check if another process is using port 8000:
```bash
lsof -i :8000
# Kill the process or change server port
collab-server --port 8001
```

## Architecture

### Components

1. **CLI** (`collab`): Rust binary using `clap` for argument parsing and `reqwest` for HTTP
2. **Server** (`collab-server`): Rust web service using `axum` and `sqlx`
3. **Database**: SQLite3 with indexed queries for fast recipient lookup

### Data Flow

```
Worker 1                    Server                    Worker 2
   |                          |                          |
   |-- POST /messages ------->|                          |
   |<------- 201 Created -----|                          |
   |                          |<---- GET /messages -----|
   |                          |------ messages[] ------->|
```

### Security Notes

This is designed for **trusted local networks** or **VPN environments**. For production use over public networks, add:

- HTTPS/TLS encryption
- Authentication tokens
- Rate limiting
- Input validation/sanitization

## Database Schema

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,          -- UUID v4
    hash TEXT NOT NULL,            -- SHA1 hash of content
    sender TEXT NOT NULL,          -- Sender instance ID
    recipient TEXT NOT NULL,       -- Recipient instance ID
    content TEXT NOT NULL,         -- Message body
    refs TEXT NOT NULL,            -- Comma-separated hash references
    timestamp TEXT NOT NULL        -- RFC3339 timestamp
);

CREATE INDEX idx_recipient_timestamp 
ON messages(recipient, timestamp DESC);
```

## License

[Your License Here]

## Contributing

[Contribution Guidelines]

## Support

For issues or questions:
- GitHub Issues: [repository-url]/issues
- Documentation: [repository-url]/docs
