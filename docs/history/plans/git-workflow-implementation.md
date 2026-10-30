# Git Workflow Implementation Guide

This guide provides step-by-step instructions for implementing the comprehensive git workflow rules and mobile KiloCode integration in the DayTrader project.

## Prerequisites

- Git repository initialized
- GitHub or GitLab account with admin access
- CI/CD platform configured (GitHub Actions or GitLab CI)
- Team members identified and roles assigned

## Phase 1: Repository Setup

### 1.1 Configure Branch Protection

#### GitHub Setup
```bash
# Create .github/workflows/branch-protection.yml
name: Branch Protection
on:
  pull_request_target:
    types: [opened, synchronize, reopened]

jobs:
  check-branch:
    runs-on: ubuntu-latest
    steps:
      - name: Check branch protection
        run: |
          if [[ "$GITHUB_HEAD_REF" == "main" || "$GITHUB_HEAD_REF" == "develop" ]]; then
            echo "Direct pushes to protected branches are not allowed"
            exit 1
          fi
```

#### GitHub Branch Protection Rules
Navigate to: Repository Settings → Branches → Branch protection rules

**For `main` branch:**
- Require pull request reviews before merging
- Require status checks to pass before merging
- Include administrators
- Restrict pushes that create matching branches
- Allow force pushes: No
- Allow deletions: No

**For `develop` branch:**
- Require pull request reviews before merging
- Require status checks to pass before merging
- Allow force pushes: No
- Allow deletions: No

### 1.2 Set Up Pre-commit Hooks

```bash
#!/bin/sh
# .git/hooks/pre-commit

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "${YELLOW}Running pre-commit checks...${NC}"

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "${RED}Error: Not in a git repository${NC}"
    exit 1
fi

# Run cargo check
echo "Running cargo check..."
if ! cargo check; then
    echo "${RED}Error: cargo check failed${NC}"
    exit 1
fi

# Run tests
echo "Running cargo test..."
if ! cargo test --lib; then
    echo "${RED}Error: cargo test failed${NC}"
    exit 1
fi

# Run clippy
echo "Running cargo clippy..."
if ! cargo clippy -- -D warnings; then
    echo "${RED}Error: cargo clippy failed${NC}"
    exit 1
fi

# Check formatting
echo "Checking code formatting..."
if ! cargo fmt -- --check; then
    echo "${RED}Error: Code formatting issues found. Run 'cargo fmt' to fix.${NC}"
    exit 1
fi

echo "${GREEN}All pre-commit checks passed!${NC}"
```

**Installation:**
```bash
chmod +x .git/hooks/pre-commit
```

### 1.3 Configure Commit Message Linting

Create `.commitlintrc.json`:
```json
{
  "extends": ["@commitlint/config-conventional"],
  "rules": {
    "type-enum": [
      2,
      "always",
      [
        "feat",
        "fix",
        "docs",
        "style",
        "refactor",
        "test",
        "chore",
        "perf",
        "ci",
        "revert"
      ]
    ],
    "scope-case": [2, "always", "lower-case"],
    "subject-case": [2, "always", "lower-case"],
    "subject-empty": [2, "never"],
    "subject-full-stop": [2, "never", "."],
    "header-max-length": [2, "always", 72],
    "scope-empty": [0, "never"]
  }
}
```

Add to `package.json` (create if needed):
```json
{
  "devDependencies": {
    "@commitlint/cli": "^17.0.0",
    "@commitlint/config-conventional": "^17.0.0",
    "husky": "^8.0.0"
  },
  "scripts": {
    "prepare": "husky install"
  }
}
```

**Install and configure:**
```bash
npm install
npm run prepare
echo 'npx commitlint --edit "$1"' > .husky/commit-msg
chmod +x .husky/commit-msg
```

## Phase 2: CI/CD Pipeline Setup

### 2.1 GitHub Actions CI Pipeline

Create `.github/workflows/ci.yml`:
```yaml
name: CI Pipeline

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Run tests
        run: cargo test --verbose

      - name: Run security audit
        run: |
          cargo install cargo-audit
          cargo audit

      - name: Generate coverage report
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml
        continue-on-error: true

  integration-test:
    runs-on: ubuntu-latest
    needs: test
    if: github.ref == 'refs/heads/develop' || github.ref == 'refs/heads/main'
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      - name: Run integration tests
        run: cargo test --test integration --verbose

      - name: Run performance benchmarks
        run: cargo bench
        continue-on-error: true
```

### 2.2 Pull Request Template

Create `.github/PULL_REQUEST_TEMPLATE.md`:
```markdown
## Description
Brief description of the changes made

## Type of Change
- [ ] 🐛 Bug fix (non-breaking change which fixes an issue)
- [ ] ✨ New feature (non-breaking change which adds functionality)
- [ ] 💥 Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] 📚 Documentation update
- [ ] 🎨 Style/code quality improvement
- [ ] ♻️ Refactor (no functional changes)
- [ ] 🧪 Test addition/modification
- [ ] 🔧 Build/CI/CD changes
- [ ] ⚡ Performance improvement
- [ ] 🔄 Revert

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests pass
- [ ] Manual testing completed
- [ ] Performance benchmarks pass

## Checklist
- [ ] Code follows established patterns and conventions
- [ ] Documentation updated (if applicable)
- [ ] No new compiler warnings
- [ ] Commit messages follow conventional format
- [ ] Mobile compatibility verified

## Screenshots (if applicable)
<!-- Add screenshots to help explain your changes -->

## Additional Notes
<!-- Any additional information or context -->

## Mobile Review Checklist
- [ ] UI changes work on mobile screens
- [ ] Touch interactions function properly
- [ ] Performance acceptable on mobile devices
- [ ] Emergency controls accessible when needed
```

## Phase 3: Mobile Integration Setup

### 3.1 Create Mobile API Service

Create `src/mobile/` directory structure:
```
src/mobile/
├── api.rs          # REST API client
├── websocket.rs    # WebSocket client
├── auth.rs         # Authentication handling
├── tasks.rs        # Task management
├── reviews.rs      # Code review functionality
├── emergency.rs    # Emergency controls
├── models.rs       # Data models
└── mod.rs
```

### 3.2 Implement Authentication

```rust
// src/mobile/auth.rs
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: u64,
    pub mfa_required: bool,
    pub mfa_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MFAResponse {
    pub mfa_required: bool,
    pub methods: Vec<String>,
}

pub struct AuthClient {
    base_url: String,
    client: reqwest::Client,
}

impl AuthClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn login(&self, request: LoginRequest) -> Result<LoginResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/auth/login", self.base_url);
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let login_response: LoginResponse = response.json().await?;
            Ok(login_response)
        } else {
            Err(format!("Login failed: {}", response.status()).into())
        }
    }

    pub async fn verify_mfa(&self, token: &str, code: &str) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/auth/mfa", self.base_url);
        let mfa_request = serde_json::json!({
            "mfa_token": token,
            "code": code
        });

        let response = self.client
            .post(&url)
            .json(&mfa_request)
            .send()
            .await?;

        if response.status().is_success() {
            let jwt_response: serde_json::Value = response.json().await?;
            if let Some(token) = jwt_response.get("token") {
                Ok(token.as_str().unwrap_or("").to_string())
            } else {
                Err("No token in MFA response".into())
            }
        } else {
            Err(format!("MFA verification failed: {}", response.status()).into())
        }
    }
}
```

### 3.3 Implement Task Management

```rust
// src/mobile/tasks.rs
use serde::{Deserialize, Serialize};
use crate::mobile::auth::AuthClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    pub title: String,
    pub description: String,
    pub r#type: String,
    pub priority: String,
    pub assignee: String,
    pub branch_name: String,
    pub estimated_hours: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: String,
    pub status: String,
    pub branch_name: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: u32,
    pub branch_name: String,
    pub assignee: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub logs: Vec<TaskLogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskLogEntry {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
    pub source: String,
}

pub struct TaskClient {
    base_url: String,
    auth_token: String,
    client: reqwest::Client,
}

impl TaskClient {
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self {
            base_url,
            auth_token,
            client: reqwest::Client::new(),
        }
    }

    pub async fn create_task(&self, request: TaskRequest) -> Result<TaskResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/tasks", self.base_url);
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let task: TaskResponse = response.json().await?;
            Ok(task)
        } else {
            Err(format!("Task creation failed: {}", response.status()).into())
        }
    }

    pub async fn get_task_status(&self, task_id: &str) -> Result<TaskStatus, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/tasks/{}", self.base_url, task_id);
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await?;

        if response.status().is_success() {
            let status: TaskStatus = response.json().await?;
            Ok(status)
        } else {
            Err(format!("Failed to get task status: {}", response.status()).into())
        }
    }

    pub async fn control_task(&self, task_id: &str, action: &str, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/tasks/{}/control", self.base_url, task_id);
        let control_request = serde_json::json!({
            "action": action,
            "reason": reason
        });

        let response = self.client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&control_request)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Task control failed: {}", response.status()).into())
        }
    }
}
```

### 3.4 Implement WebSocket Client

```rust
// src/mobile/websocket.rs
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    #[serde(rename = "task.status.changed")]
    TaskStatusChanged {
        task_id: String,
        status: String,
        progress: u32,
        timestamp: u64,
    },
    #[serde(rename = "task.log.entry")]
    TaskLogEntry {
        task_id: String,
        entry: TaskLogEntry,
    },
    #[serde(rename = "emergency.alert")]
    EmergencyAlert {
        incident_id: String,
        severity: String,
        title: String,
        message: String,
        timestamp: u64,
        requires_action: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskLogEntry {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
    pub source: String,
}

pub struct WebSocketClient {
    url: String,
    auth_token: String,
}

impl WebSocketClient {
    pub fn new(url: String, auth_token: String) -> Self {
        Self { url, auth_token }
    }

    pub async fn connect<F>(&self, message_handler: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(WebSocketMessage) + Send + 'static,
    {
        let url = Url::parse(&format!("{}?token={}", self.url, self.auth_token))?;

        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                            message_handler(ws_message);
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(e) => eprintln!("WebSocket error: {}", e),
                    _ => {}
                }
            }
        });

        Ok(())
    }
}
```

## Phase 4: Emergency Controls Setup

### 4.1 Create Emergency Control Module

```rust
// src/mobile/emergency.rs
use serde::{Deserialize, Serialize};
use crate::mobile::auth::AuthClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct EmergencyHaltRequest {
    pub reason: String,
    pub scope: String,
    pub severity: String,
    pub notify_team: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmergencyResponse {
    pub id: String,
    pub status: String,
    pub actions_taken: Vec<EmergencyAction>,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmergencyAction {
    pub r#type: String,
    pub target: String,
    pub status: String,
    pub details: String,
}

pub struct EmergencyClient {
    base_url: String,
    auth_token: String,
    client: reqwest::Client,
}

impl EmergencyClient {
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self {
            base_url,
            auth_token,
            client: reqwest::Client::new(),
        }
    }

    pub async fn emergency_halt(&self, request: EmergencyHaltRequest) -> Result<EmergencyResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/emergency/halt", self.base_url);
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let emergency_response: EmergencyResponse = response.json().await?;
            Ok(emergency_response)
        } else {
            Err(format!("Emergency halt failed: {}", response.status()).into())
        }
    }

    pub async fn rollback_deployment(&self, environment: &str, target_version: &str, reason: &str) -> Result<EmergencyResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/v1/emergency/rollback", self.base_url);
        let rollback_request = serde_json::json!({
            "environment": environment,
            "target_version": target_version,
            "reason": reason,
            "confirm_impact": true
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .json(&rollback_request)
            .send()
            .await?;

        if response.status().is_success() {
            let emergency_response: EmergencyResponse = response.json().await?;
            Ok(emergency_response)
        } else {
            Err(format!("Rollback failed: {}", response.status()).into())
        }
    }
}
```

## Phase 5: Testing and Validation

### 5.1 Create Integration Tests

```rust
// tests/mobile_integration.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::block_on;

    #[test]
    fn test_task_creation() {
        block_on(async {
            // Mock server setup would go here
            // Test task creation flow
        });
    }

    #[test]
    fn test_websocket_connection() {
        block_on(async {
            // Test WebSocket connection and message handling
        });
    }

    #[test]
    fn test_emergency_controls() {
        block_on(async {
            // Test emergency halt functionality
        });
    }
}
```

### 5.2 Performance Testing

```rust
// benches/mobile_performance.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_task_creation(c: &mut Criterion) {
    // Benchmark task creation performance
}

fn bench_websocket_message_handling(c: &mut Criterion) {
    // Benchmark WebSocket message processing
}

criterion_group!(benches, bench_task_creation, bench_websocket_message_handling);
criterion_main!(benches);
```

## Phase 6: Documentation and Training

### 6.1 Create User Guides

Create `docs/mobile-integration.md`:
```markdown
# Mobile Integration Guide

This guide explains how to use the mobile KiloCode integration for remote development.

## Getting Started

1. Install the KiloCode mobile app
2. Configure your repository connection
3. Set up authentication and MFA

## Task Management

### Creating Tasks
1. Open the app and navigate to Tasks
2. Tap "Create Task"
3. Fill in task details
4. Assign to team member
5. Monitor progress in real-time

### Monitoring Progress
- View task status and progress
- Check CI/CD pipeline status
- Review logs and error messages
- Receive push notifications for updates

## Code Review

### Reviewing PRs
1. Navigate to Pull Requests
2. Select a PR to review
3. View diff and changes
4. Add comments and suggestions
5. Approve or request changes

### Mobile-Optimized Interface
- Syntax-highlighted code diffs
- Touch-friendly navigation
- Offline comment drafting
- Quick approval/rejection actions
```

### 6.2 Training Materials

Create `docs/training/` directory with:
- Video tutorials
- Quick reference guides
- Emergency response procedures
- Best practices documentation

## Implementation Checklist

### Repository Setup
- [ ] Branch protection rules configured
- [ ] Pre-commit hooks installed and tested
- [ ] CI/CD pipelines operational
- [ ] PR templates created and working

### Mobile Integration
- [ ] Authentication module implemented
- [ ] Task management client working
- [ ] WebSocket client functional
- [ ] Emergency controls tested
- [ ] API error handling complete

### Quality Assurance
- [ ] Unit tests for mobile clients passing
- [ ] Integration tests with mock server working
- [ ] Performance benchmarks established
- [ ] Security audit completed

### Documentation
- [ ] User guides created and published
- [ ] API documentation complete
- [ ] Training materials prepared
- [ ] Emergency procedures documented

### Deployment
- [ ] Mobile API server deployed
- [ ] WebSocket server operational
- [ ] Authentication system configured
- [ ] Monitoring and alerting active

This implementation guide provides everything needed to set up comprehensive git workflows and mobile KiloCode integration for the DayTrader project.