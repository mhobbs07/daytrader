# Mobile KiloCode Integration API

This document specifies the API endpoints and WebSocket events for mobile KiloCode integration with the DayTrader project.

## Overview

The Mobile KiloCode API provides remote access to development workflows, enabling mobile users to:
- Initiate and monitor development tasks
- Review and approve code changes
- Access emergency controls for critical situations
- Receive real-time notifications and status updates

## Authentication

### JWT Token Authentication

All API requests require a valid JWT token in the Authorization header:

```
Authorization: Bearer <jwt_token>
```

### Multi-Factor Authentication

Mobile access requires MFA verification:

```typescript
interface MFAResponse {
  mfa_required: boolean;
  mfa_token?: string;
  methods: ('sms' | 'app' | 'email')[];
}

interface MFAVerifyRequest {
  mfa_token: string;
  code: string;
  method: 'sms' | 'app' | 'email';
}
```

### Session Management

```typescript
interface SessionInfo {
  user_id: string;
  roles: string[];
  permissions: string[];
  expires_at: number;
  mfa_verified: boolean;
}
```

## REST API Endpoints

### Task Management

#### Create Task
```http
POST /api/v1/tasks
Authorization: Bearer <token>
Content-Type: application/json

{
  "title": "Add risk validation for position sizing",
  "description": "Implement validation logic for minimum position sizes",
  "type": "feature",
  "priority": "high",
  "assignee": "user_id",
  "branch_name": "feature/add-risk-validation",
  "estimated_hours": 4
}
```

**Response:**
```typescript
interface TaskResponse {
  id: string;
  status: 'created' | 'running' | 'completed' | 'failed';
  branch_name: string;
  created_at: number;
  updated_at: number;
}
```

#### Get Task Status
```http
GET /api/v1/tasks/{task_id}
Authorization: Bearer <token>
```

**Response:**
```typescript
interface TaskStatus {
  id: string;
  title: string;
  status: TaskStatusType;
  progress: number; // 0-100
  branch_name: string;
  assignee: string;
  created_at: number;
  updated_at: number;
  logs: TaskLogEntry[];
  ci_status?: CIStatus;
}

type TaskStatusType =
  | 'created'
  | 'branch_created'
  | 'ci_running'
  | 'ci_passed'
  | 'ci_failed'
  | 'ready_for_review'
  | 'reviewing'
  | 'approved'
  | 'merged'
  | 'completed'
  | 'failed'
  | 'cancelled';

interface TaskLogEntry {
  timestamp: number;
  level: 'info' | 'warn' | 'error';
  message: string;
  source: 'git' | 'ci' | 'system';
}

interface CIStatus {
  status: 'pending' | 'running' | 'passed' | 'failed';
  url?: string;
  checks: CICheck[];
}

interface CICheck {
  name: string;
  status: 'pending' | 'running' | 'passed' | 'failed';
  url?: string;
}
```

#### Control Task Execution
```http
PUT /api/v1/tasks/{task_id}/control
Authorization: Bearer <token>
Content-Type: application/json

{
  "action": "pause" | "resume" | "cancel",
  "reason": "Emergency: critical bug found"
}
```

#### List Tasks
```http
GET /api/v1/tasks?status=running&assignee=user_id&limit=20&offset=0
Authorization: Bearer <token>
```

### Code Review

#### List Pull Requests
```http
GET /api/v1/prs?status=open&reviewer=user_id&limit=10
Authorization: Bearer <token>
```

**Response:**
```typescript
interface PRListResponse {
  prs: PullRequest[];
  total: number;
  has_more: boolean;
}

interface PullRequest {
  id: number;
  title: string;
  description: string;
  author: string;
  branch: string;
  base_branch: string;
  status: 'open' | 'closed' | 'merged';
  created_at: number;
  updated_at: number;
  reviewers: Reviewer[];
  checks: CICheck[];
  additions: number;
  deletions: number;
  changed_files: number;
}

interface Reviewer {
  user_id: string;
  status: 'pending' | 'approved' | 'changes_requested' | 'dismissed';
  reviewed_at?: number;
}
```

#### Get PR Details
```http
GET /api/v1/prs/{pr_id}
Authorization: Bearer <token>
```

**Response:**
```typescript
interface PRDetails extends PullRequest {
  diff: DiffEntry[];
  comments: PRComment[];
  commits: Commit[];
}

interface DiffEntry {
  filename: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  additions: number;
  deletions: number;
  patch?: string; // Diff content
}

interface PRComment {
  id: number;
  author: string;
  body: string;
  path?: string; // File path for line comments
  line?: number;
  created_at: number;
  resolved: boolean;
}

interface Commit {
  sha: string;
  message: string;
  author: string;
  date: number;
}
```

#### Submit Review
```http
POST /api/v1/prs/{pr_id}/review
Authorization: Bearer <token>
Content-Type: application/json

{
  "event": "APPROVE" | "REQUEST_CHANGES" | "COMMENT",
  "body": "Looks good, just one small suggestion...",
  "comments": [
    {
      "path": "src/risk/engine.rs",
      "line": 42,
      "body": "Consider adding error handling here"
    }
  ]
}
```

### Emergency Controls

#### Emergency Halt
```http
POST /api/v1/emergency/halt
Authorization: Bearer <token>
Content-Type: application/json

{
  "reason": "Critical production issue detected",
  "scope": "trading_system" | "ci_pipeline" | "deployment",
  "severity": "critical" | "high" | "medium",
  "notify_team": true
}
```

**Response:**
```typescript
interface EmergencyResponse {
  id: string;
  status: 'initiated' | 'executing' | 'completed' | 'failed';
  actions_taken: EmergencyAction[];
  timestamp: number;
}

interface EmergencyAction {
  type: 'halt_trading' | 'cancel_ci' | 'rollback_deployment' | 'isolate_system';
  target: string;
  status: 'pending' | 'completed' | 'failed';
  details: string;
}
```

#### Rollback Deployment
```http
POST /api/v1/emergency/rollback
Authorization: Bearer <token>
Content-Type: application/json

{
  "environment": "production" | "staging",
  "target_version": "v1.2.0",
  "reason": "Performance regression detected",
  "confirm_impact": true
}
```

#### Get Emergency Status
```http
GET /api/v1/emergency/status
Authorization: Bearer <token>
```

**Response:**
```typescript
interface EmergencyStatus {
  active_incidents: EmergencyIncident[];
  system_health: SystemHealth;
  recent_actions: EmergencyAction[];
}

interface EmergencyIncident {
  id: string;
  type: string;
  severity: 'critical' | 'high' | 'medium';
  status: 'active' | 'resolved' | 'mitigated';
  started_at: number;
  description: string;
  affected_systems: string[];
}

interface SystemHealth {
  overall_status: 'healthy' | 'degraded' | 'critical';
  components: ComponentHealth[];
}

interface ComponentHealth {
  name: string;
  status: 'healthy' | 'degraded' | 'down';
  last_check: number;
  message?: string;
}
```

### System Monitoring

#### Get Dashboard Data
```http
GET /api/v1/dashboard?components=ci,deployments,health
Authorization: Bearer <token>
```

**Response:**
```typescript
interface DashboardData {
  ci_status: CIStatus[];
  deployments: DeploymentStatus[];
  health: SystemHealth;
  alerts: Alert[];
  timestamp: number;
}

interface DeploymentStatus {
  environment: string;
  version: string;
  status: 'deploying' | 'deployed' | 'failed' | 'rolled_back';
  deployed_at: number;
  health_checks: HealthCheck[];
}

interface Alert {
  id: string;
  level: 'info' | 'warning' | 'error' | 'critical';
  title: string;
  message: string;
  timestamp: number;
  acknowledged: boolean;
}

interface HealthCheck {
  name: string;
  status: 'passing' | 'failing' | 'unknown';
  last_run: number;
  duration_ms: number;
}
```

## WebSocket Events

### Connection

```typescript
// Client connects with JWT token
const ws = new WebSocket('wss://api.kilocode.dev/ws?token=<jwt_token>');

// Server confirms connection
{
  "type": "connection_established",
  "user_id": "user123",
  "session_id": "session456"
}
```

### Task Events

```typescript
// Task status updates
{
  "type": "task.status.changed",
  "task_id": "task123",
  "status": "ci_running",
  "progress": 65,
  "timestamp": 1640995200000
}

// Task log updates
{
  "type": "task.log.entry",
  "task_id": "task123",
  "entry": {
    "timestamp": 1640995200000,
    "level": "info",
    "message": "Running cargo test...",
    "source": "ci"
  }
}
```

### PR Events

```typescript
// PR status changes
{
  "type": "pr.status.changed",
  "pr_id": 123,
  "status": "ready_for_review",
  "timestamp": 1640995200000
}

// New PR comments
{
  "type": "pr.comment.added",
  "pr_id": 123,
  "comment": {
    "id": 456,
    "author": "reviewer123",
    "body": "Please add error handling",
    "path": "src/main.rs",
    "line": 42
  }
}
```

### CI/CD Events

```typescript
// CI status updates
{
  "type": "ci.status.changed",
  "pipeline_id": "pipeline123",
  "status": "running",
  "stage": "test",
  "timestamp": 1640995200000
}

// CI check completed
{
  "type": "ci.check.completed",
  "pipeline_id": "pipeline123",
  "check_name": "unit_tests",
  "status": "passed",
  "duration_ms": 45000,
  "url": "https://github.com/org/repo/actions/runs/123"
}
```

### Emergency Events

```typescript
// Emergency alert
{
  "type": "emergency.alert",
  "incident_id": "incident123",
  "severity": "critical",
  "title": "Production System Down",
  "message": "Trading engine unresponsive",
  "timestamp": 1640995200000,
  "requires_action": true
}

// Emergency action taken
{
  "type": "emergency.action.completed",
  "incident_id": "incident123",
  "action": "halt_trading",
  "status": "completed",
  "timestamp": 1640995200000
}
```

### System Health Events

```typescript
// Health status change
{
  "type": "health.status.changed",
  "component": "trading_engine",
  "status": "degraded",
  "message": "High latency detected",
  "timestamp": 1640995200000
}

// Deployment status
{
  "type": "deployment.status.changed",
  "environment": "production",
  "status": "deployed",
  "version": "v1.2.0",
  "timestamp": 1640995200000
}
```

## Error Handling

### Standard Error Response

```typescript
interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: any;
    timestamp: number;
  };
}
```

### Common Error Codes

- `AUTH_INVALID_TOKEN`: Invalid or expired JWT token
- `AUTH_MFA_REQUIRED`: MFA verification required
- `AUTH_INSUFFICIENT_PERMISSIONS`: User lacks required permissions
- `TASK_NOT_FOUND`: Specified task does not exist
- `EMERGENCY_ALREADY_ACTIVE`: Emergency action already in progress
- `RATE_LIMIT_EXCEEDED`: Too many requests
- `SYSTEM_UNAVAILABLE`: Service temporarily unavailable

## Rate Limiting

### Rate Limits by Endpoint Type

```typescript
const RATE_LIMITS = {
  // Task management
  'POST /api/v1/tasks': { requests: 10, window: 'minute' },
  'GET /api/v1/tasks': { requests: 60, window: 'minute' },

  // Code review
  'GET /api/v1/prs': { requests: 30, window: 'minute' },
  'POST /api/v1/prs/*/review': { requests: 5, window: 'minute' },

  // Emergency (higher limits for critical actions)
  'POST /api/v1/emergency/*': { requests: 20, window: 'minute' },

  // General
  'GET /api/v1/dashboard': { requests: 60, window: 'minute' }
};
```

### Rate Limit Headers

```http
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 59
X-RateLimit-Reset: 1640995260
X-RateLimit-Retry-After: 60  // When limit exceeded
```

## Mobile-Specific Considerations

### Offline Support

- **Task Queue**: Queue actions when offline, sync when reconnected
- **Status Cache**: Cache recent status updates for offline viewing
- **Emergency Mode**: Critical alerts work offline via push notifications

### Battery Optimization

- **WebSocket Batching**: Batch multiple events to reduce wake-ups
- **Selective Updates**: Only send relevant notifications
- **Background Sync**: Efficient background data synchronization

### Security Features

- **Biometric Authentication**: Device-level biometric verification
- **Remote Wipe**: Ability to remotely disable mobile access
- **Session Timeouts**: Automatic logout after inactivity
- **Device Tracking**: Monitor and control authorized devices

## Implementation Checklist

### API Endpoints
- [ ] Authentication endpoints implemented
- [ ] Task management endpoints working
- [ ] Code review endpoints functional
- [ ] Emergency control endpoints tested
- [ ] Monitoring endpoints operational

### WebSocket Events
- [ ] Real-time task updates working
- [ ] PR event notifications functional
- [ ] CI/CD status updates implemented
- [ ] Emergency alerts configured
- [ ] Health monitoring events active

### Mobile Integration
- [ ] JWT token handling implemented
- [ ] MFA flow working
- [ ] WebSocket connection stable
- [ ] Offline queue functional
- [ ] Push notifications configured

### Security & Compliance
- [ ] Rate limiting enforced
- [ ] Audit logging active
- [ ] Emergency access controls tested
- [ ] Session management verified
- [ ] Mobile security features enabled

This API specification provides the foundation for comprehensive mobile access to development workflows, enabling remote task management, code review, and emergency response capabilities.