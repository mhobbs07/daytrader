# Deployment Guide

## Prerequisites
- Docker 20.10+
- Docker Compose 2.0+
- GitLab Runner (for CI/CD)

## Environment Variables
Set these in GitLab CI/CD settings or .env file:
- `ALPACA_API_KEY` - Alpaca API key
- `ALPACA_API_SECRET` - Alpaca API secret
- `SSH_PRIVATE_KEY` - SSH key for deployment
- `DEPLOY_USER` - Deployment user
- `STAGING_HOST` - Staging server hostname
- `PROD_HOST` - Production server hostname

## Local Development with Docker
```bash
# Build image
docker build -t daytrader:local .

# Run with docker-compose
docker-compose up -d

# View logs
docker-compose logs -f daytrader

# Stop
docker-compose down
```

## GitLab CI/CD Pipeline
1. Push to `develop` branch triggers:
   - Tests
   - Security audit
   - Docker build
   - Auto-deploy to staging

2. Push to `main` branch triggers:
   - Tests
   - Docker build
   - Manual deploy to production

## Production Deployment
1. Merge to `main` branch
2. Go to GitLab CI/CD pipelines
3. Click "Play" button on `deploy_production` job
4. Confirm deployment