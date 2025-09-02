### EARLY DEVELOPMENT STAGE

> dBranch is currently in the early development stage. While the core functionality is implemented, there may be bugs and missing features. We welcome contributions and feedback from the community to help improve the project.

### *PLEASE DO NOT USE IN PRODUCTION*
---

## 🌿 dBranch - PostgreSQL Database Branching System

dBranch is a database branching system for PostgreSQL that enables developers to create, manage, and switch between multiple database branches effortlessly. Built with Rust, it leverages BTRFS snapshots for efficient storage and Docker containers for isolated database instances.

## 🎯 Key Features

- **Instant Database Branching**: Create lightweight database branches using BTRFS copy-on-write (COW) snapshots
- **Resource Efficient**: Branches share common data blocks, minimizing storage overhead
- **Isolated Environments**: Each branch runs in its own Docker container with dedicated ports
- **Transparent Proxy**: Seamlessly switch between branches without changing connection strings
- **Project Management**: Organize multiple database projects with their own branch hierarchies

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         dBranch System                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                     CLI Interface                        │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐   │   │
│  │  │   init   │  │  create  │  │   use    │  │  stop   │   │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └─────────┘   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                      Proxy Layer                         │   │
│  │                                                          │   │
│  │               PostgreSQL Proxy (Port 5432)               │   │
│  │                           ↓                              │   │
│  │            Routes to active branch container             │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  Container Layer                         │   │
│  │                                                          │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │   │
│  │  │  Postgres   │  │  Postgres   │  │  Postgres   │       │   │
│  │  │  main:5433  │  │ branch:5434 │  │ branch:5435 │       │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Storage Layer                         │   │
│  │                                                          │   │
│  │  Project A                    Project B                  │   │
│  │  ┌────────────────────┐      ┌────────────────────┐      │   │
│  │  │  BTRFS Filesystem  │      │  BTRFS Filesystem  │      │   │
│  │  │  ┌──────────────┐  │      │  ┌──────────────┐  │      │   │
│  │  │  │     main     │  │      │  │     main     │  │      │   │
│  │  │  ├──────────────┤  │      │  ├──────────────┤  │      │   │
│  │  │  │   branch-1   │  │      │  │   feature-x  │  │      │   │
│  │  │  ├──────────────┤  │      │  ├──────────────┤  │      │   │
│  │  │  │   branch-2   │  │      │  │   hotfix-y   │  │      │   │
│  │  │  └──────────────┘  │      │  └──────────────┘  │      │   │
│  │  └────────────────────┘      └────────────────────┘      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## 📋 Prerequisites

- **Operating System**: Linux with BTRFS support
- **Docker**: Installed and running
- **Rust**: 1.70+ (for building from source)
- **Available disk space**: Minimum 5GB per project


## Sudo privileges

Some operations require sudo privileges. For example, commands like `mount`, `losetup`, and `btrfs` need elevated permissions. Therefore, dBranch may occasionally request sudo access, but it will always display the command on the screen.


## 🚀 Installation

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/dbranch.git
cd dbranch

# Build the project
cargo build --release

# Install the binary (optional)
sudo cp target/release/dbranch /usr/local/bin/
```

## 📖 Usage

### Initialize a Project

Create a new dBranch project with a PostgreSQL instance:

```bash
dbranch init
```
or

```bash
dbranch init --name my_project --port 5432
```

This creates:
- A BTRFS filesystem for the project
- A main PostgreSQL container
- Configuration files in `.config/`

### Create a Branch

Create a new database branch from the current state:

```bash
dbranch create feature-branch
```

Or create from a specific source branch:

```bash
dbranch create hotfix --source main
```

### Switch Between Branches

Change the active branch (proxy will route to this branch):

```bash
dbranch use feature-branch
```

### View Project Status

Check the current project and active branch:

```bash
dbranch status
```

### List All Branches

```bash
dbranch list
```

### Start the Proxy Server

Start the PostgreSQL proxy that routes connections to the active branch:

```bash
dbranch start
```

The proxy listens on port 5432 by default and forwards connections to the active branch's container.

### Stop All Containers

Stop all running containers and unmount filesystems:

```bash
dbranch stop
```

### Resume After Stop

Resume all containers and remount filesystems:

```bash
dbranch resume
```

### Delete a Branch

```bash
dbranch delete branch-name
```

### Delete an Entire Project

```bash
dbranch delete-project project-name
```

### Set Default Project

```bash
dbranch set-default project-name
```

## ⚙️ Configuration

Configuration is stored in `.config/dbranch.config.json`:

```json
{
  "api_port": 8080,
  "proxy_port": 5432,
  "port_min": 5433,
  "port_max": 5500,
  "mount_point": "/mnt/dbranch",
  "default_project": "my_project",
  "postgres_config": {
    "user": "postgres",
    "password": "postgres",
    "database": "dbranch"
  },
  "projects": ["my_project"]
}
```

### Environment Variables

- `DBRANCH_CONFIG`: Custom configuration directory (default: `.config`)
- `DBRANCH_API_PORT`: API server port
- `DBRANCH_PROXY_PORT`: Proxy server port
- `DBRANCH_MOUNT_POINT`: BTRFS mount point

## 🔧 How It Works

1. **Project Initialization**: Creates a sparse file and formats it as a BTRFS filesystem
2. **Branch Creation**: Uses BTRFS snapshots to create instant, space-efficient copies
3. **Container Management**: Each branch runs in an isolated Docker container
4. **Proxy Routing**: A TCP proxy forwards connections to the active branch's container
5. **Data Persistence**: BTRFS volumes are mounted into containers, ensuring data survives restarts

### Development Workflow

```bash
# Initialize project
dbranch init --name myapp

# Create feature branch
dbranch create feature-new-schema

# Switch to feature branch
dbranch use feature-new-schema

# Make database changes...
# Test your application...

# Create another branch for experiments
dbranch create experiment --source feature-new-schema

# Switch back to main
dbranch use main
```
## 🔒 Security Considerations

- Requires sudo privileges for BTRFS operations
- Each container runs in an isolated network (`dbranch-network`)
- Default PostgreSQL credentials should be changed in production
- Consider firewall rules for exposed ports

## 📝 License

MIT License - See LICENSE file for details

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 🔗 Links

- [Issue Tracker](https://github.com/joshuapassos/dbranch/issues)