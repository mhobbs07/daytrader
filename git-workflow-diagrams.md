# Git Workflow Diagrams

This document contains visual diagrams for the git workflow processes and mobile KiloCode integration.

## Branching Strategy Flow

```mermaid
graph TD
    A[main<br/>Production] --> B[release/v*.*.*<br/>Release Prep]
    B --> A
    B --> C[develop<br/>Integration]

    C --> D[feature/*<br/>Feature Dev]
    D --> C

    A --> E[hotfix/*<br/>Critical Fix]
    E --> A
    E --> C

    subgraph "Branch Types"
        F[main<br/>Protected<br/>Deploy Ready]
        G[develop<br/>Integration<br/>CI Required]
        H[feature/*<br/>Development<br/>Delete After Merge]
        I[release/*<br/>Stabilization<br/>Version Bump]
        J[hotfix/*<br/>Emergency<br/>Direct to Main]
    end

    style A fill:#e1f5fe
    style C fill:#f3e5f5
    style D fill:#e8f5e8
    style B fill:#fff3e0
    style E fill:#ffebee
```

## Feature Development Workflow

```mermaid
sequenceDiagram
    participant Dev as Developer
    participant Local as Local Git
    participant Remote as GitHub/GitLab
    participant CI as CI/CD Pipeline
    participant Mobile as KiloCode Mobile

    Dev->>Local: git checkout -b feature/new-feature
    Dev->>Local: Make atomic commits
    Dev->>Remote: git push origin feature/new-feature

    Remote->>CI: Trigger CI checks
    CI->>Remote: Report status

    Dev->>Remote: Create Pull Request
    Remote->>Mobile: Notify mobile reviewers

    Mobile->>Remote: Review and approve PR
    Remote->>CI: Run full test suite

    CI->>Remote: All checks pass
    Remote->>Local: Merge to develop
    Local->>Local: Delete feature branch

    Note over Mobile: Mobile can monitor<br/>progress and approve<br/>from anywhere
```

## Release Workflow

```mermaid
stateDiagram-v2
    [*] --> FeatureComplete: All features merged to develop
    FeatureComplete --> ReleaseBranch: Create release/v1.2.0 from develop
    ReleaseBranch --> Testing: Run integration tests
    Testing --> BugFixes: Fix any issues found
    BugFixes --> Testing
    Testing --> VersionBump: Update version numbers
    VersionBump --> Changelog: Generate release notes
    Changelog --> MergeMain: Merge to main branch
    MergeMain --> TagRelease: Create git tag v1.2.0
    TagRelease --> DeployProd: Trigger production deployment
    DeployProd --> MergeDevelop: Merge release back to develop
    MergeDevelop --> [*]

    note right of ReleaseBranch : Mobile approval required
    note right of DeployProd : Emergency rollback available
```

## Mobile KiloCode Integration Architecture

```mermaid
graph TB
    subgraph "Mobile App"
        MA[Mobile Interface<br/>React Native]
        MT[Task Management]
        MR[Code Review]
        ME[Emergency Controls]
    end

    subgraph "KiloCode API Server"
        API[REST API<br/>Express.js]
        WS[WebSocket Server<br/>Real-time Updates]
        AUTH[Authentication<br/>JWT + MFA]
        AUDIT[Audit Logging]
    end

    subgraph "Repository Services"
        GIT[Git Operations<br/>Branch/PR Management]
        CI[CI/CD Control<br/>Pipeline Management]
        DEPLOY[Deployment Control<br/>Environment Mgmt]
        MONITOR[Health Monitoring<br/>System Status]
    end

    MA --> API
    MA --> WS
    API --> AUTH
    WS --> AUDIT
    API --> GIT
    API --> CI
    API --> DEPLOY
    API --> MONITOR

    MT -.-> GIT
    MR -.-> GIT
    ME -.-> CI
    ME -.-> DEPLOY
    ME -.-> MONITOR

    style MA fill:#e3f2fd
    style API fill:#f3e5f5
    style GIT fill:#e8f5e8
```

## Emergency Response Workflow

```mermaid
flowchart TD
    A[Incident Detected] --> B{Classification}
    B -->|Level 1 Critical| C[Immediate Alert]
    B -->|Level 2 High| D[Urgent Alert]
    B -->|Level 3 Medium| E[Scheduled Response]

    C --> F[Mobile Emergency Dashboard]
    D --> F
    E --> G[Standard Process]

    F --> H{Assessment}
    H -->|System Down| I[Execute Emergency Halt]
    H -->|Security Breach| J[Isolate Systems]
    H -->|Data Corruption| K[Activate Backup]

    I --> L[Containment Actions]
    J --> L
    K --> L

    L --> M[Recovery Procedures]
    M --> N[Communication Updates]
    N --> O[Post-mortem Analysis]
    O --> P[Documentation & Prevention]

    G --> Q[Normal Incident Response]
    Q --> R[Resolution]
    R --> S[Close Incident]

    style C fill:#ffebee
    style I fill:#ffebee
    style F fill:#fff3e0
```

## Pull Request Review Process

```mermaid
stateDiagram-v2
    [*] --> Draft: PR Created
    Draft --> Ready: Mark as ready for review
    Ready --> Review: Assign reviewers

    Review --> Changes: Request changes
    Changes --> Ready

    Review --> Approved: All approvals received
    Approved --> CI: Run final CI checks

    CI --> Failed: CI checks fail
    Failed --> Changes

    CI --> Passed: All checks pass
    Passed --> Merge: Auto-merge or manual merge
    Merge --> [*]

    note right of Review : Mobile reviewers can<br/>approve/reject remotely
    note right of CI : Emergency stop available<br/>if issues detected
```

## Mobile Task Management Flow

```mermaid
sequenceDiagram
    participant User as Mobile User
    participant App as KiloCode App
    participant API as KiloCode API
    participant Git as Git Repository
    participant CI as CI/CD System

    User->>App: Initiate task "Add risk validation"
    App->>API: POST /api/tasks
    API->>Git: Create feature branch
    Git->>API: Branch created
    API->>CI: Trigger initial checks
    CI->>API: Status updates
    API->>App: Real-time status via WebSocket
    App->>User: Progress notifications

    User->>App: Monitor progress
    App->>API: GET /api/tasks/{id}/status
    API->>App: Current status & logs

    User->>App: Emergency stop needed
    App->>API: POST /api/emergency/halt
    API->>CI: Cancel running pipelines
    API->>Git: Cleanup if needed
    API->>App: Confirmation
    App->>User: Emergency action completed
```

## Quality Gate Pipeline

```mermaid
flowchart LR
    subgraph "Pre-commit"
        PC1[cargo check]
        PC2[cargo test]
        PC3[cargo clippy]
        PC4[cargo fmt --check]
    end

    subgraph "CI Pipeline"
        CI1[Lint & Format]
        CI2[Unit Tests]
        CI3[Integration Tests]
        CI4[Security Scan]
        CI5[Performance Tests]
        CI6[Build Artifacts]
    end

    subgraph "Quality Metrics"
        QM1[Test Coverage >80%]
        QM2[Zero Critical Vulns]
        QM3[Performance Baseline]
        QM4[Mobile Compatibility]
    end

    PC1 --> CI1
    PC2 --> CI2
    PC3 --> CI1
    PC4 --> CI1

    CI1 --> CI2
    CI2 --> CI3
    CI3 --> CI4
    CI4 --> CI5
    CI5 --> CI6

    CI2 --> QM1
    CI4 --> QM2
    CI5 --> QM3
    CI6 --> QM4

    style PC1 fill:#e8f5e8
    style CI1 fill:#e3f2fd
    style QM1 fill:#fff3e0
```

## Mobile Emergency Dashboard

```mermaid
graph TD
    subgraph "Emergency Dashboard"
        ED[Emergency Status]
        SA[System Alerts]
        QA[Quick Actions]
        SD[System Dashboard]
        CL[Communication Log]
    end

    subgraph "Quick Actions"
        SH[System Halt]
        RB[Rollback Deploy]
        IS[Isolate Component]
        AT[Alert Team]
    end

    subgraph "System Status"
        HS[Health Status]
        AI[Active Incidents]
        RP[Recovery Progress]
        TL[Timeline]
    end

    ED --> SA
    ED --> QA
    ED --> SD
    ED --> CL

    QA --> SH
    QA --> RB
    QA --> IS
    QA --> AT

    SD --> HS
    SD --> AI
    SD --> RP
    SD --> TL

    style ED fill:#ffebee
    style SH fill:#ffebee
    style HS fill:#e8f5e8
```

## Commit Message Validation

```mermaid
flowchart TD
    A[Commit Message] --> B{Format Check}
    B -->|Invalid| C[Reject Commit]
    B -->|Valid| D{Type Check}

    D -->|Invalid| C
    D -->|Valid| E{Scope Check}

    E -->|Invalid| C
    E -->|Valid| F{Description Check}

    F -->|Invalid| C
    F -->|Valid| G[Accept Commit]

    C --> H[Show Error Message]
    H --> I[Prompt User to Fix]

    G --> J[Proceed with Commit]

    style C fill:#ffebee
    style G fill:#e8f5e8
```

## Mobile Authentication Flow

```mermaid
sequenceDiagram
    participant User as Mobile User
    participant App as KiloCode App
    participant API as KiloCode API
    participant Auth as Auth Service
    participant DB as User Database

    User->>App: Login attempt
    App->>API: POST /auth/login
    API->>Auth: Validate credentials
    Auth->>DB: Check user record
    DB->>Auth: User data
    Auth->>API: MFA challenge required
    API->>App: MFA prompt

    User->>App: Enter MFA code
    App->>API: POST /auth/mfa
    API->>Auth: Verify MFA code
    Auth->>API: MFA valid
    API->>Auth: Generate JWT token
    Auth->>API: Signed JWT
    API->>App: Authentication successful

    App->>User: Access granted

    Note over App,API: All subsequent requests<br/>include JWT token
```

---

## Legend

### Branch Colors
- 🔵 **Main**: Production ready
- 🟣 **Develop**: Integration branch
- 🟢 **Feature**: Development work
- 🟠 **Release**: Stabilization
- 🔴 **Hotfix**: Emergency fixes

### Status Indicators
- ✅ **Pass**: Successful operation
- ❌ **Fail**: Failed operation
- ⏳ **Pending**: In progress
- 🚨 **Alert**: Requires attention

### Mobile Capabilities
- 📱 **Full Access**: Complete mobile functionality
- 📊 **Read-only**: Status monitoring only
- 🚨 **Emergency**: Emergency controls only
- 🔒 **Restricted**: Limited access

These diagrams provide visual guidance for all git workflow processes and mobile integration points. They serve as quick references for team members and mobile users.