### EARLY DEVELOPMENT STAGE

> dBranch is currently in the early development stage. While the core functionality is implemented, there may be bugs and missing features. We welcome contributions and feedback from the community to help improve the project.

### *PLEASE DO NOT USE IN PRODUCTION*
---

## 🌿 dBranch - PostgreSQL Database Branching System

dBranch is a database branching system designed for PostgreSQL that empowers developers to effortlessly create, manage, and switch between multiple database branches.

Its key features include Instant Database Branching, which allows for the creation of lightweight branches using copy-on-write. This approach makes the system highly Resource Efficient, as all branches share common data blocks, dramatically minimizing storage overhead.

For isolation and stability, each branch operates within its own Isolated Environment—a dedicated Docker container that ensures no interference between branches and provides unique network ports.

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
│  │  │   start  │  │  create  │  │   use    │  │  usage  │   │   │
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
│  │  ┌─────────────────────────────────────────────────────┐ │   │
│  │  │                 COW Filesystem                      │ │   │
│  │  │                ┌──────────────┐                     │ │   │
│  │  │                │     main     │                     │ │   │
│  │  │                ├──────────────┤                     │ │   │
│  │  │                │   branch-1   │                     │ │   │
│  │  │                ├──────────────┤                     │ │   │
│  │  │                │   branch-2   │                     │ │   │
│  │  │                └──────────────┘                     │ │   │
│  │  └─────────────────────────────────────────────────────┘ │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Prerequisites

- **Operating System**: Any with CoW filesystem support (e.g., Linux with BTRFS)
- **Docker**: Installed and running
- **Rust**: 1.70+ (for building from source)


## Usage

Create a new dBranch project:

```bash
dbranch init
```

Edit `.dbranch.config.json` to set your configuration.

Start the first branch (main):

```bash
dbranch init-postgres
```

Create a new branch:

```bash
dbranch create <branch-name> # e.g. dbranch create feature-new-schema
```

## TODO
- [X] Replace BTRFS module with direct syscall implementation
- [X] Add support for additional filesystems with CoW support (e.g., ZFS)
- [ ] MacOS support
- [ ] Windows support
- [ ] Improve Postgres configuration to share more files between branches (e.g Disable auto vacuum and wall recycling)
- [ ] Add tests
- [ ] Improve error handling and messages
- [ ] Sync with remote postgres (optional)
- [ ] Web interface to manage branches
