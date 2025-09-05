### EARLY DEVELOPMENT STAGE

> dBranch is currently in the early development stage. While the core functionality is implemented, there may be bugs and missing features. We welcome contributions and feedback from the community to help improve the project.

### *PLEASE DO NOT USE IN PRODUCTION*
---

## 🌿 dBranch - PostgreSQL Database Branching System

dBranch is a database branching system designed for PostgreSQL that empowers developers to effortlessly create, manage, and switch between multiple database branches. Built with Rust (my first project in Rust), it leverages the powerful capabilities of BTRFS snapshots for efficient storage management and utilizes Docker containers to provide fully isolated database instances.

Its key features include Instant Database Branching, which allows for the creation of lightweight branches using BTRFS's copy-on-write (COW) snapshots. This approach makes the system highly Resource Efficient, as all branches inherently share common data blocks, dramatically minimizing storage overhead. For isolation and stability, each branch operates within its own Isolated Environment—a dedicated Docker container that ensures no interference between branches and provides unique network ports.

Furthermore, dBranch includes a Transparent Proxy that enables seamless context switching between different database branches without requiring any changes to the application's connection string, streamlining the development workflow.

---

## Architecture

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

## Prerequisites

- **Operating System**: Linux with BTRFS support
- **Docker**: Installed and running
- **Rust**: 1.70+ (for building from source)


## Sudo privileges

Some operations require sudo privileges. For example, commands like `mount`, `losetup`, and `btrfs` need elevated permissions. Therefore, dBranch may occasionally request sudo access, but it will always display the command on the screen.


## Usage

Create a new dBranch project with a PostgreSQL instance:

```bash
dbranch init
```

## TODO
- [ ] Rewrite BTRFS module :) 
- [ ] Improve Postgres configuration to share more files between branches (e.g Disable auto vacuum and wall recycling)
- [ ] Add tests
- [ ] Improve error handling and messages
- [ ] Sync with remote postgres (optional)
- [ ] Web interface to manage branches

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
