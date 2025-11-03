# MigChat Server

A lightweight, real-time chat server built with Rust, using Axum web framework and SQLite for in-memory storage.

## Features

- **User Account Management**: Create accounts with unique usernames
- **Token-based Authentication**: Secure session management with bearer tokens
- **Direct Messaging**: Send messages to other users by username
- **Message Retrieval**: Fetch all messages (sent and received)
- **Conversation List**: View all conversations with metadata
- **In-Memory Storage**: Fast SQLite database stored in memory

## Tech Stack

- **Rust** - Systems programming language
- **Axum** - Web application framework
- **SQLite** - In-memory database via sqlx
- **Tokio** - Async runtime
- **Docker** - Containerization for easy deployment

## API Endpoints

### Health Check
```
GET /health
```
Returns: `OK`

### Create Account
```
POST /api/account/create
Content-Type: application/json

{
  "username": "your_username",
  "password": "your_password"
}
```

**Response:**
```json
{
  "token": "generated_auth_token",
  "user_id": 1,
  "username": "your_username"
}
```

**Error Responses:**
- `400 Bad Request` - Invalid input (empty username/password)
- `409 Conflict` - Username already exists

### Send Message
```
POST /api/messages/send
Authorization: Bearer YOUR_TOKEN
Content-Type: application/json

{
  "to_username": "recipient_username",
  "content": "Your message here"
}
```

**Response:**
```json
{
  "message_id": 1,
  "created_at": "2025-11-03T12:00:00Z"
}
```

**Error Responses:**
- `401 Unauthorized` - Invalid or missing token
- `404 Not Found` - Recipient user not found
- `400 Bad Request` - Empty message content

### Get Messages
```
GET /api/messages
Authorization: Bearer YOUR_TOKEN
```

**Response:**
```json
[
  {
    "id": 1,
    "from_username": "sender",
    "to_username": "recipient",
    "content": "Hello!",
    "created_at": "2025-11-03T12:00:00Z"
  }
]
```

Returns all messages where you are either the sender or recipient, sorted by creation time (newest first).

### Get Conversations
```
GET /api/conversations
Authorization: Bearer YOUR_TOKEN
```

**Response:**
```json
[
  {
    "username": "other_user",
    "last_message": "Last message content",
    "last_message_time": "2025-11-03T12:00:00Z",
    "unread_count": 5
  }
]
```

Returns a list of all your conversations with metadata.

## Local Development

### Prerequisites
- Rust 1.70 or higher
- Cargo

### Running Locally

1. Clone the repository:
```bash
git clone https://github.com/migchat/server.git
cd server
```

2. Build and run:
```bash
cargo build --release
cargo run
```

The server will start on `http://localhost:3000`

### Environment Variables

- `PORT` - Server port (default: 3000)
- `RUST_LOG` - Logging level (default: `migchat_server=debug,tower_http=debug`)

## Deployment Options

### Option 1: Deploy to Fly.io (Recommended - Free Tier)

Fly.io offers a generous free tier perfect for this application. Deploy in minutes:

#### Prerequisites
Install the Fly CLI:
```bash
# macOS/Linux
curl -L https://fly.io/install.sh | sh

# Windows (PowerShell)
pwsh -Command "iwr https://fly.io/install.ps1 -useb | iex"
```

#### Deploy Steps

1. **Login to Fly.io**:
```bash
fly auth login
```

2. **Launch your app** (from the server directory):
```bash
cd server
fly launch
```
When prompted:
- Choose an app name (or accept the generated one)
- Select a region (choose closest to you)
- Accept the default settings

3. **Deploy**:
```bash
fly deploy
```

Your app will be live at `https://your-app-name.fly.dev` in a few minutes!

#### Useful Fly.io Commands

```bash
# View logs
fly logs

# Open app in browser
fly open

# Check app status
fly status

# SSH into the app
fly ssh console

# Scale the app (free tier: max 3 VMs with 256MB each)
fly scale count 1

# View app info
fly info
```

#### Configuration

The `fly.toml` file contains your app configuration:
- **Region**: `sea` (Seattle) - change to your preferred region
- **Memory**: 256MB (free tier)
- **Auto-scaling**: Stops when inactive, starts on request
- **Health checks**: Monitors `/health` endpoint

#### Continuous Deployment with GitHub Actions

This repository includes a GitHub Actions workflow that automatically deploys to Fly.io whenever you push to the `main` branch.

**Setup Instructions:**

1. **Get your Fly.io API Token**:
```bash
fly auth token
```

2. **Add the token to GitHub Secrets**:
   - Go to your GitHub repository
   - Navigate to **Settings** → **Secrets and variables** → **Actions**
   - Click **New repository secret**
   - Name: `FLY_API_TOKEN`
   - Value: Paste your Fly.io token
   - Click **Add secret**

3. **Initial Deployment**:
Before GitHub Actions can deploy, you need to create the app on Fly.io once:
```bash
fly launch --no-deploy
```

4. **Automatic Deployments**:
Now every push to `main` will automatically deploy to Fly.io! Check the **Actions** tab in your GitHub repository to see deployment status.

**Workflow Features:**
- Triggers on every push to `main`
- Prevents concurrent deployments
- Uses remote-only builds (builds on Fly.io's infrastructure)
- Shows deployment status in GitHub

### Option 2: Docker Deployment

#### Build and Run with Docker

```bash
docker build -t migchat-server .
docker run -p 3000:3000 migchat-server
```

#### Using Docker Compose

```bash
docker-compose up -d
```

To stop:
```bash
docker-compose down
```

## Deploying to Hetzner

### Option 1: Using Docker

1. SSH into your Hetzner server:
```bash
ssh root@your-server-ip
```

2. Install Docker (if not already installed):
```bash
curl -fsSL https://get.docker.com -o get-docker.sh
sh get-docker.sh
```

3. Clone and deploy:
```bash
git clone https://github.com/migchat/server.git
cd server
docker-compose up -d
```

4. Configure firewall (if needed):
```bash
ufw allow 3000/tcp
```

### Option 2: Direct Binary Deployment

1. Build on your local machine:
```bash
cargo build --release
```

2. Copy binary to server:
```bash
scp target/release/migchat-server root@your-server-ip:/opt/migchat/
```

3. Create systemd service on the server:
```bash
sudo nano /etc/systemd/system/migchat.service
```

Add:
```ini
[Unit]
Description=MigChat Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/migchat
ExecStart=/opt/migchat/migchat-server
Restart=always
Environment="PORT=3000"

[Install]
WantedBy=multi-user.target
```

4. Enable and start:
```bash
sudo systemctl enable migchat
sudo systemctl start migchat
sudo systemctl status migchat
```

### Nginx Reverse Proxy (Recommended)

```nginx
server {
    listen 80;
    server_name your-domain.com;

    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

## Security Notes

- **In-Memory Database**: All data is lost when the server restarts. Not suitable for production use without modifications.
- **Password Storage**: Passwords are hashed using bcrypt with default cost factor.
- **CORS**: Currently allows all origins. Restrict this in production.
- **HTTPS**: Always use HTTPS in production. Consider using Let's Encrypt with nginx.

## Architecture

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │
       │ HTTP/JSON
       │
┌──────▼──────────────────────┐
│   Axum Web Server           │
│   - CORS Layer              │
│   - Auth Middleware         │
│   - Route Handlers          │
└──────┬──────────────────────┘
       │
┌──────▼──────────────────────┐
│   SQLite (In-Memory)        │
│   - users                   │
│   - sessions                │
│   - messages                │
└─────────────────────────────┘
```

## License

MIT

## Contributing

Pull requests are welcome! For major changes, please open an issue first to discuss what you would like to change.
