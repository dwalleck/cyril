# Cyril Documentation Index

## 🤖 Instructions for AI Assistants

**This file is your primary entry point for understanding the Cyril codebase.**

This index provides rich metadata about each documentation file, enabling you to quickly find relevant information without reading all documents. Use this file to:

1. **Understand what information exists** - Browse the table of contents to see what's documented
2. **Find relevant files** - Use the metadata tags to identify which files contain information you need
3. **Navigate efficiently** - Jump directly to the specific document that answers your question
4. **Understand relationships** - See how different aspects of the system connect

### How to Use This Index

**For Architecture Questions:**
- Start with `architecture.md` for system design
- Reference `components.md` for specific component details
- Check `workflows.md` for process flows

**For Implementation Questions:**
- Check `components.md` for component APIs
- Reference `interfaces.md` for protocol details
- Review `data_models.md` for data structures

**For Integration Questions:**
- Start with `interfaces.md` for APIs
- Check `workflows.md` for integration patterns
- Reference `dependencies.md` for external libraries

**For Troubleshooting:**
- Review `review_notes.md` for known gaps
- Check `workflows.md` for error handling
- Reference `architecture.md` for design decisions

---

## 📊 Project Overview

**Project:** Cyril  
**Version:** 0.1.0-alpha.1  
**Status:** Alpha - functional but under active development  
**Language:** Rust (Edition 2021)  
**Size:** Large (429 files, ~6,783 LOC)  
**Architecture:** Two-crate workspace (binary + library)

**Purpose:** Cross-platform TUI client for Kiro CLI via Agent Client Protocol (ACP)

**Key Features:**
- Streaming markdown rendering with syntax highlighting
- Cross-platform support (Linux native, Windows via WSL bridge)
- JSON-configurable hook system
- Slash command system with autocomplete
- Real-time tool call visualization
- Approval workflow for file operations

---

## 📚 Documentation Files

### codebase_info.md
**Purpose:** Basic project information and statistics

**Contains:**
- Project metadata (name, version, license)
- Technology stack and dependencies
- Codebase statistics (files, LOC, functions)
- Workspace structure
- Platform support details
- Key features overview
- Build and installation information

**Use When:**
- Getting started with the project
- Understanding project scope
- Checking technology stack
- Finding build instructions

**Metadata Tags:** `#overview` `#statistics` `#dependencies` `#build` `#installation`

---

### architecture.md
**Purpose:** System architecture and design patterns

**Contains:**
- High-level architecture diagrams
- Architectural layers (Presentation, Protocol, Platform, Hooks, Capabilities)
- Communication flow between components
- Path translation architecture (Windows ↔ WSL)
- Hook system architecture
- Data flow diagrams
- Design principles and decisions
- Performance and security considerations

**Use When:**
- Understanding system design
- Making architectural decisions
- Adding new major features
- Troubleshooting cross-component issues
- Understanding Windows/WSL bridge

**Metadata Tags:** `#architecture` `#design` `#patterns` `#layers` `#communication` `#path-translation` `#hooks` `#security` `#performance`

**Key Diagrams:**
- High-level architecture graph
- Path translation flow
- Hook execution architecture
- User input → Agent response sequence
- Tool call execution sequence

---

### components.md
**Purpose:** Detailed component documentation

**Contains:**
- All components in both crates (cyril, cyril-core)
- Component responsibilities and LOC counts
- Key structs, methods, and functions
- Component interactions and dependencies
- Test coverage information
- Usage patterns and examples

**Use When:**
- Understanding specific components
- Finding component APIs
- Locating functionality
- Understanding component relationships
- Checking test coverage

**Metadata Tags:** `#components` `#api` `#modules` `#functions` `#structs` `#tests` `#usage`

**Major Components Documented:**
- Binary crate: main.rs, app.rs, commands.rs, file_completer.rs, ui/*
- Library crate: protocol/*, platform/*, hooks/*, capabilities/*, session.rs

**Most Complex Components:**
- `commands.rs` (905 LOC) - Command system
- `platform/terminal.rs` (361 LOC) - Terminal management
- `hooks/config.rs` (452 LOC) - Hook configuration

---

### interfaces.md
**Purpose:** APIs, interfaces, and integration points

**Contains:**
- Agent Client Protocol (ACP) implementation
- All ACP methods with request/response formats
- Internal module interfaces (KiroClient, AgentProcess, etc.)
- Path translation API
- Terminal manager API
- Hook system API
- Session context API
- Event system interface
- Extension points (custom hooks, Kiro extensions)
- Integration patterns

**Use When:**
- Implementing ACP methods
- Understanding protocol communication
- Adding new capabilities
- Creating custom hooks
- Integrating with external systems
- Understanding event flow

**Metadata Tags:** `#api` `#protocol` `#acp` `#interfaces` `#integration` `#extensions` `#events`

**Key APIs:**
- ACP methods: requestPermission, readTextFile, writeTextFile, createTerminal, etc.
- KiroClient interface
- Path translation functions
- Hook system traits
- Event types

---

### data_models.md
**Purpose:** Data structures and models

**Contains:**
- Protocol data models (JSON-RPC, ACP messages)
- Application state models (App, ChatState, InputState, etc.)
- Platform models (path translation, terminal management)
- Hook models (configuration, execution)
- Command models (parsing, execution)
- Extension models (Kiro extensions)
- Event models (protocol, internal, extension)
- Data flow diagrams
- Data validation rules
- Memory management strategies

**Use When:**
- Understanding data structures
- Implementing new features
- Serializing/deserializing data
- Managing application state
- Understanding data flow
- Debugging data issues

**Metadata Tags:** `#data` `#models` `#structures` `#state` `#serialization` `#validation` `#memory`

**Key Models:**
- ACP message types
- Permission request/response
- Terminal models
- Session context
- Chat state and messages
- Tool call tracking

---

### workflows.md
**Purpose:** Key processes and workflows

**Contains:**
- User workflows (startup, sending messages, file completion, autocomplete)
- Protocol workflows (request-response, streaming, tool calls)
- File operation workflows (read, write with hooks)
- Terminal workflows (creation, execution, output capping)
- Session workflows (creation, loading, model selection)
- Path translation workflows (Windows ↔ WSL)
- Hook execution workflows
- Error handling workflows
- Performance optimization workflows
- Shutdown workflow

**Use When:**
- Understanding process flows
- Implementing new workflows
- Debugging workflow issues
- Optimizing performance
- Understanding error handling
- Tracing execution paths

**Metadata Tags:** `#workflows` `#processes` `#flows` `#sequences` `#user-interaction` `#protocol` `#hooks` `#errors` `#performance`

**Key Workflows:**
- Application startup sequence
- Message send and streaming response
- Tool call execution with approval
- File write with hooks
- Terminal creation and execution
- Path translation (bidirectional)

---

### dependencies.md
**Purpose:** External dependencies and their usage

**Contains:**
- All external dependencies with versions
- Dependency purposes and features used
- Integration points in codebase
- Performance considerations
- Security considerations
- Dependency graph
- Version constraints and update strategy
- License compatibility

**Use When:**
- Understanding external libraries
- Updating dependencies
- Adding new dependencies
- Troubleshooting dependency issues
- Security auditing
- License compliance

**Metadata Tags:** `#dependencies` `#libraries` `#versions` `#security` `#licenses` `#updates`

**Major Dependencies:**
- agent-client-protocol (0.9) - ACP protocol
- tokio (1.x) - Async runtime
- ratatui (0.29) - Terminal UI
- crossterm (0.28) - Terminal manipulation
- pulldown-cmark (0.12) - Markdown parsing
- syntect (5.x) - Syntax highlighting

---

### review_notes.md
**Purpose:** Documentation quality assessment and gaps

**Contains:**
- Consistency check results
- Completeness check results
- Well-documented areas
- Areas needing more detail
- Language support limitations
- Documentation quality assessment
- Recommendations by priority
- Documentation maintenance plan

**Use When:**
- Understanding documentation gaps
- Planning documentation improvements
- Identifying missing information
- Prioritizing documentation work
- Understanding limitations

**Metadata Tags:** `#review` `#gaps` `#quality` `#recommendations` `#maintenance`

**Key Findings:**
- Documentation is internally consistent
- Core technical documentation is comprehensive
- Gaps in user-facing tutorials and troubleshooting
- Gaps in contributor guidelines
- Limited coverage of configuration files

---

## 🔍 Quick Reference Guide

### By Question Type

**"How does X work?"**
→ Start with `workflows.md`, then `architecture.md`

**"What is the API for X?"**
→ Check `interfaces.md`, then `components.md`

**"What data structure represents X?"**
→ Look in `data_models.md`

**"Which component handles X?"**
→ Search `components.md`

**"How do I integrate with X?"**
→ Check `interfaces.md` for integration patterns

**"What dependencies does X use?"**
→ Review `dependencies.md`

**"Is X documented?"**
→ Check `review_notes.md` for known gaps

### By Component

**Protocol Communication:**
- Architecture: `architecture.md` (Protocol Layer)
- Components: `components.md` (protocol/client.rs, protocol/transport.rs)
- Interfaces: `interfaces.md` (ACP Interface)
- Workflows: `workflows.md` (Protocol Workflows)

**UI Rendering:**
- Architecture: `architecture.md` (Presentation Layer)
- Components: `components.md` (ui/ modules)
- Data Models: `data_models.md` (UI Models)
- Workflows: `workflows.md` (User Workflows)

**Path Translation:**
- Architecture: `architecture.md` (Platform Abstraction Layer)
- Components: `components.md` (platform/path.rs)
- Interfaces: `interfaces.md` (Path Translation Interface)
- Workflows: `workflows.md` (Path Translation Workflows)

**Hook System:**
- Architecture: `architecture.md` (Hook System)
- Components: `components.md` (hooks/*)
- Interfaces: `interfaces.md` (Hook System Interface)
- Workflows: `workflows.md` (Hook Execution Workflows)

**Terminal Management:**
- Components: `components.md` (platform/terminal.rs)
- Interfaces: `interfaces.md` (Terminal Manager Interface)
- Workflows: `workflows.md` (Terminal Workflows)

### By Task

**Adding a New Feature:**
1. Review `architecture.md` for design principles
2. Check `components.md` for related components
3. Review `interfaces.md` for relevant APIs
4. Check `workflows.md` for similar patterns
5. Review `review_notes.md` for known gaps

**Debugging an Issue:**
1. Check `workflows.md` for expected behavior
2. Review `components.md` for component details
3. Check `data_models.md` for data structures
4. Review `architecture.md` for design decisions

**Understanding Integration:**
1. Start with `interfaces.md` for APIs
2. Check `workflows.md` for integration patterns
3. Review `dependencies.md` for external libraries
4. Check `architecture.md` for integration points

---

## 🏗️ Codebase Structure Reference

```
cyril/
├── crates/
│   ├── cyril/              # Binary crate (TUI application)
│   │   ├── src/
│   │   │   ├── main.rs     # Entry point (245 LOC)
│   │   │   ├── app.rs      # Event loop (459 LOC)
│   │   │   ├── commands.rs # Command system (905 LOC) ⭐
│   │   │   ├── file_completer.rs # File completion (183 LOC)
│   │   │   ├── tui.rs      # Terminal setup (26 LOC)
│   │   │   ├── event.rs    # Event types (29 LOC)
│   │   │   └── ui/         # UI components
│   │   │       ├── input.rs      # Input field (299 LOC)
│   │   │       ├── chat.rs       # Message display (287 LOC)
│   │   │       ├── markdown.rs   # Markdown rendering (243 LOC)
│   │   │       ├── highlight.rs  # Syntax highlighting (116 LOC)
│   │   │       ├── tool_calls.rs # Tool display (291 LOC)
│   │   │       ├── approval.rs   # Approval UI (203 LOC)
│   │   │       ├── picker.rs     # Selection UI (171 LOC)
│   │   │       ├── toolbar.rs    # Status bar (139 LOC)
│   │   │       └── cache.rs      # LRU cache (89 LOC)
│   │   └── Cargo.toml
│   └── cyril-core/         # Library crate (protocol & platform)
│       ├── src/
│       │   ├── lib.rs      # Public API (12 LOC)
│       │   ├── session.rs  # Session state (216 LOC)
│       │   ├── event.rs    # Event types (77 LOC)
│       │   ├── kiro_ext.rs # Kiro extensions (196 LOC)
│       │   ├── protocol/   # ACP protocol
│       │   │   ├── client.rs    # ACP client (358 LOC)
│       │   │   └── transport.rs # Process mgmt (161 LOC)
│       │   ├── platform/   # Platform abstraction
│       │   │   ├── path.rs      # Path translation (306 LOC)
│       │   │   └── terminal.rs  # Terminal mgmt (361 LOC) ⭐
│       │   ├── hooks/      # Hook system
│       │   │   ├── types.rs     # Hook registry (101 LOC)
│       │   │   ├── config.rs    # Hook loading (452 LOC) ⭐
│       │   │   └── builtins.rs  # Built-in hooks (41 LOC)
│       │   └── capabilities/ # File operations
│       │       └── fs.rs        # File I/O (73 LOC)
│       └── Cargo.toml
├── docs/               # Documentation and plans
├── .kiro/              # Kiro CLI configuration
│   ├── skills/         # Development skills
│   ├── agents/         # Custom agents
│   ├── hooks/          # Git hooks
│   └── settings/       # LSP configuration
├── .claude/            # Claude AI configuration
└── .agents/            # AI-generated documentation (this directory)
    └── summary/
        ├── index.md           # This file
        ├── codebase_info.md   # Project overview
        ├── architecture.md    # System architecture
        ├── components.md      # Component details
        ├── interfaces.md      # APIs and interfaces
        ├── data_models.md     # Data structures
        ├── workflows.md       # Process flows
        ├── dependencies.md    # External dependencies
        └── review_notes.md    # Documentation review

⭐ = Most complex components (>400 LOC)
```

---

## 📈 Documentation Statistics

- **Total Documentation Files:** 9
- **Total Documentation Size:** ~50,000 words
- **Diagrams:** 30+ Mermaid diagrams
- **Components Documented:** 38 major components
- **APIs Documented:** 15+ interfaces
- **Workflows Documented:** 20+ workflows
- **Dependencies Documented:** 15+ external libraries

---

## 🔄 Documentation Maintenance

**Last Updated:** 2026-03-03  
**Baseline Commit:** 7b8366b1  
**Documentation Version:** 1.0

**Update Frequency:**
- **Per Release:** Update all documentation
- **Per Major Change:** Update affected documents
- **Monthly:** Review for accuracy

**How to Update:**
1. Run codebase analysis tool
2. Review changes since last update
3. Update affected documentation files
4. Update this index if new files added
5. Run consistency and completeness checks

---

## 💡 Tips for AI Assistants

1. **Start with this index** - Don't read all files, use metadata to find what you need
2. **Use metadata tags** - Tags help you quickly identify relevant files
3. **Follow cross-references** - Documents reference each other for related information
4. **Check review notes** - Known gaps are documented, don't assume everything is covered
5. **Use diagrams** - Visual representations often explain concepts better than text
6. **Understand the structure** - Two-crate architecture is fundamental to understanding the system
7. **Consider the platform** - Windows/WSL bridge is a key architectural feature
8. **Remember the protocol** - ACP is the communication foundation

---

## 🎯 Common Queries and Where to Find Answers

| Query | Primary File | Secondary File |
|-------|-------------|----------------|
| How does the TUI work? | architecture.md | components.md (ui/) |
| How does ACP communication work? | interfaces.md | workflows.md (Protocol) |
| How are paths translated? | workflows.md (Path Translation) | architecture.md (Platform Layer) |
| How do hooks work? | architecture.md (Hook System) | workflows.md (Hook Execution) |
| What is the command system? | components.md (commands.rs) | workflows.md (User Workflows) |
| How are terminals managed? | components.md (terminal.rs) | workflows.md (Terminal Workflows) |
| What data structures exist? | data_models.md | components.md |
| How do I add a new feature? | architecture.md (Design Principles) | review_notes.md (Recommendations) |
| What dependencies are used? | dependencies.md | codebase_info.md |
| What's not documented? | review_notes.md | - |

---

## 📞 Getting Help

If you can't find what you need in this documentation:

1. **Check review_notes.md** - Your question might be in a known gap
2. **Search the codebase** - Use code search tools to find implementations
3. **Check git history** - Recent changes might not be documented yet
4. **Ask the maintainers** - Some knowledge might not be written down yet

---

## 🚀 Next Steps

After reading this index:

1. **For general understanding:** Read `codebase_info.md` and `architecture.md`
2. **For implementation work:** Read `components.md` and `interfaces.md`
3. **For specific tasks:** Use the Quick Reference Guide above
4. **For troubleshooting:** Check `workflows.md` and `review_notes.md`

Remember: This index is your map. Use it to navigate efficiently rather than reading everything sequentially.
