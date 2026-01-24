# Git Workflow Rules & Mobile KiloCode Integration

## Overview

This document establishes comprehensive git workflow rules for the DayTrader project, designed to support both traditional development workflows and mobile-first remote development using KiloCode. The rules ensure atomic commits, consistent branching strategies, and mobile-accessible emergency controls.

## Table of Contents

1. [Branching Strategy](#branching-strategy)
2. [Commit Guidelines](#commit-guidelines)
3. [Pull Request Process](#pull-request-process)
4. [Release Workflow](#release-workflow)
5. [Mobile KiloCode Integration](#mobile-kilocode-integration)
6. [Emergency Protocols](#emergency-protocols)
7. [Quality Gates](#quality-gates)

## Branching Strategy

### Branch Types

```
main (production) ← develop (integration) ← feature branches
                              ↑
                        release branches
                              ↓
                       hotfix branches
```

#### Main Branch (`main`)
- **Purpose**: Production-ready code
- **Protection**: Requires PR approval, CI/CD must pass
- **Merges**: Only from release branches or hotfixes
- **Mobile Access**: Emergency rollback capability

#### Develop Branch (`develop`)
- **Purpose**: Integration branch for features
- **Protection**: Requires PR approval for merges
- **Merges**: Feature branches merge here
- **Mobile Access**: Status monitoring and emergency halt

#### Feature Branches (`feature/*`)
- **Naming**: `feature/description-with-hyphens`
- **Source**: Always branch from `develop`
- **Target**: Always merge back to `develop`
- **Lifetime**: Delete after merge
- **Mobile Access**: Task initiation and progress tracking

#### Release Branches (`release/v*.*.*`)
- **Naming**: `release/v1.2.3`
- **Source**: Branch from `develop` when ready for release
- **Target**: Merge to both `main` and `develop`
- **Purpose**: Final testing and release preparation
- **Mobile Access**: Release approval and deployment control

#### Hotfix Branches (`hotfix/*`)
- **Naming**: `hotfix/critical-issue-description`
- **Source**: Branch from `main` for critical production issues
- **Target**: Merge to both `main` and `develop`
- **Priority**: Highest priority, bypasses normal process
- **Mobile Access**: Emergency intervention required

### Branch Naming Conventions

```bash
# Features
feature/add-risk-management-engine
feature/implement-pattern-recognition
feature/mobile-api-endpoints

# Releases
release/v1.2.0
release/v2.0.0-beta

# Hotfixes
hotfix/critical-security-patch
hotfix/trading-engine-crash-fix

# Never use:
main, develop, master, dev, staging, production
```

## Commit Guidelines

### Atomic Commits

Each commit must represent **one logical change**:

#### ✅ Good Examples
```bash
feat: add risk validation for position sizing
fix: correct decimal precision in P&L calculations
docs: update API documentation for trading endpoints
refactor: extract common pattern detection utilities
test: add integration tests for broker connectivity
```

#### ❌ Bad Examples
```bash
update code and fix bugs  # Too vague, multiple changes
feat: add risk management, fix UI, update docs  # Multiple changes
WIP: working on risk engine  # Not ready for commit
```

### Commit Message Format

```
type(scope): description

[optional body]

[optional footer]
```

#### Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding/modifying tests
- `chore`: Maintenance tasks
- `perf`: Performance improvements
- `ci`: CI/CD changes
- `revert`: Revert previous commit

#### Scopes (Trading Platform Specific)
- `risk`: Risk management
- `pattern`: Pattern recognition
- `broker`: Broker integration
- `execution`: Trade execution
- `config`: Configuration
- `db`: Database
- `api`: API endpoints
- `mobile`: Mobile integration

#### Examples
```bash
feat(risk): implement position size validation
fix(broker): handle Alpaca API rate limits correctly
docs(api): add mobile endpoint documentation
refactor(pattern): extract common confidence calculations
test(risk): add edge cases for PDT rule enforcement
perf(db): optimize query for pattern statistics
```

### Commit Best Practices

1. **Test Before Commit**: All tests must pass locally
2. **Single Responsibility**: One change per commit
3. **Descriptive Messages**: Explain what and why, not just what
4. **Reference Issues**: Link to GitHub/GitLab issues when applicable
5. **Mobile-Friendly**: Messages readable on small screens

## Pull Request Process

### PR Creation

1. **Branch Ready**: Feature complete and tested
2. **Tests Pass**: All CI checks green
3. **Documentation Updated**: Code changes documented
4. **Self-Review**: Author reviews own code first

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update
- [ ] Performance improvement

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist
- [ ] Code follows style guidelines
- [ ] Documentation updated
- [ ] No new warnings
- [ ] Mobile compatibility verified

## Mobile Review Notes
[Any specific mobile integration considerations]
```

### Review Process

#### Required Reviews
- **Code Review**: Technical review by team member
- **Testing Review**: QA verification
- **Mobile Review**: Mobile compatibility check (if applicable)

#### Review Criteria
- **Functionality**: Code works as intended
- **Performance**: No performance regressions
- **Security**: No security vulnerabilities
- **Mobile**: Works with mobile KiloCode interface
- **Documentation**: Changes properly documented

### Mobile PR Review

Mobile reviewers can:
- View diff and comments
- Approve/reject changes
- Request modifications
- Add review comments
- Access emergency controls if issues detected

## Release Workflow

### Version Numbering

Follows [Semantic Versioning](https://semver.org/):

```
MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]
```

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Process

1. **Feature Complete**: All planned features merged to `develop`
2. **Create Release Branch**: `git checkout -b release/v1.2.0 develop`
3. **Final Testing**: Integration tests and manual testing
4. **Version Bump**: Update version numbers in code
5. **Changelog**: Generate release notes
6. **Merge to Main**: `git checkout main && git merge release/v1.2.0`
7. **Tag Release**: `git tag -a v1.2.0 -m "Release v1.2.0"`
8. **Deploy**: Trigger production deployment
9. **Merge Back**: `git checkout develop && git merge release/v1.2.0`
10. **Clean Up**: Delete release branch

### Mobile Release Control

Mobile interface provides:
- Release approval workflow
- Deployment status monitoring
- Emergency rollback capability
- Version compatibility checking

## Mobile KiloCode Integration

### Architecture Overview

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Mobile App    │────│   KiloCode API   │────│   Repository    │
│                 │    │                  │    │                 │
│ • Task Mgmt     │    │ • REST/WebSocket │    │ • Git Commands  │
│ • Code Review   │    │ • Authentication  │    │ • CI/CD Control│
│ • Emergency Ctrl│    │ • Real-time Sync │    │ • Deployments   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Mobile Capabilities

#### Task Management
- **Initiate Tasks**: Start development tasks remotely
- **Monitor Progress**: Real-time status updates
- **Control Execution**: Pause/resume/cancel tasks
- **View Logs**: Access execution logs and errors

#### Code Review
- **PR Access**: View and review pull requests
- **Diff Viewing**: Code changes with syntax highlighting
- **Comment System**: Add review comments and suggestions
- **Approval Workflow**: Approve/reject changes

#### Emergency Controls
- **Build Cancellation**: Stop running CI/CD pipelines
- **Deployment Rollback**: Revert to previous versions
- **System Halt**: Emergency stop trading systems
- **Alert System**: Critical issue notifications

### Security Considerations

#### Authentication
- **Multi-factor**: Required for all mobile access
- **Session Management**: Secure token-based sessions
- **Audit Trail**: All mobile actions logged

#### Authorization
- **Role-based**: Different permissions for different roles
- **Emergency Access**: Elevated permissions for critical situations
- **Time-based**: Restricted access during certain hours

### API Design

#### REST Endpoints
```typescript
// Task Management
POST   /api/v1/tasks              // Create task
GET    /api/v1/tasks/{id}         // Get task status
PUT    /api/v1/tasks/{id}/control // Control task execution

// Code Review
GET    /api/v1/prs                // List PRs
POST   /api/v1/prs/{id}/review    // Submit review
PUT    /api/v1/prs/{id}/approve   // Approve PR

// Emergency
POST   /api/v1/emergency/halt     // Emergency stop
POST   /api/v1/emergency/rollback // Rollback deployment
```

#### WebSocket Events
```typescript
// Real-time updates
task.status.changed
pr.review.submitted
emergency.alert.triggered
build.status.updated
```

## Emergency Protocols

### Emergency Classification

#### Level 1: Critical (Immediate Action Required)
- Production system down
- Security breach
- Data corruption
- Trading system malfunction

#### Level 2: High (Urgent Response)
- Performance degradation
- Failed deployments
- CI/CD pipeline failures
- Database issues

#### Level 3: Medium (Scheduled Response)
- Test failures
- Documentation issues
- Minor bugs

### Emergency Response Process

1. **Detection**: Automated monitoring or manual report
2. **Alert**: Notify on-call personnel via mobile
3. **Assessment**: Quick evaluation of impact and scope
4. **Containment**: Isolate affected systems
5. **Recovery**: Execute recovery procedures
6. **Communication**: Update stakeholders
7. **Post-mortem**: Analyze and document incident

### Mobile Emergency Interface

#### Quick Actions
- **System Halt**: Immediate stop all trading activities
- **Rollback**: Revert to last known good state
- **Isolate**: Disconnect problematic components
- **Alert Team**: Notify all team members

#### Status Dashboard
- **System Health**: Real-time status of all components
- **Active Incidents**: Current emergencies and responses
- **Recovery Progress**: Step-by-step recovery status
- **Communication Log**: Incident response timeline

## Quality Gates

### Pre-commit Hooks

```bash
#!/bin/sh
# Pre-commit hook
cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

### CI/CD Pipeline

#### GitHub Actions/GitLab CI Stages
1. **Lint**: Code formatting and style checks
2. **Test**: Unit and integration tests
3. **Security**: Vulnerability scanning
4. **Build**: Compile and package
5. **Deploy**: Environment-specific deployment

#### Quality Metrics
- **Test Coverage**: Minimum 80% coverage required
- **Performance**: No regression in benchmarks
- **Security**: Zero critical/high vulnerabilities
- **Mobile**: Compatible with mobile interface

### Mobile Quality Verification

Mobile interface includes:
- **Test Results**: View CI/CD test outcomes
- **Coverage Reports**: Code coverage visualization
- **Security Scan**: Vulnerability assessment results
- **Performance Metrics**: Benchmark comparisons

## Implementation Checklist

### Git Workflow Setup
- [ ] Branch protection rules configured
- [ ] Pre-commit hooks installed
- [ ] CI/CD pipelines operational
- [ ] PR templates created
- [ ] Commit message linting enabled

### Mobile Integration
- [ ] KiloCode API endpoints implemented
- [ ] Mobile authentication configured
- [ ] Emergency controls tested
- [ ] Real-time notifications working

### Documentation
- [ ] Workflow rules documented
- [ ] Mobile integration guide created
- [ ] Emergency protocols documented
- [ ] Training materials prepared

### Training
- [ ] Team workflow training completed
- [ ] Mobile interface training completed
- [ ] Emergency response drills conducted
- [ ] Process documentation reviewed

---

## Quick Reference

### Daily Workflow
1. `git checkout develop && git pull`
2. `git checkout -b feature/your-feature-name`
3. Make atomic commits with proper messages
4. Push and create PR when ready
5. Address review feedback
6. Merge when approved

### Mobile Emergency
1. Open KiloCode mobile app
2. Navigate to emergency dashboard
3. Assess situation
4. Execute appropriate emergency action
5. Communicate with team

### Commit Message Template
```
feat(risk): implement position size validation

- Add validation for minimum position sizes
- Prevent orders below risk thresholds
- Update error messages for clarity

Closes #123
```

This document serves as the comprehensive guide for git workflows and mobile integration. All team members must follow these rules to ensure code quality, system reliability, and mobile accessibility.