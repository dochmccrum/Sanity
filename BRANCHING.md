# Git Branching Strategy

This project uses a simplified Git Flow workflow.

## Branch Structure

### `main` (Production)
- Stable, release-ready code
- Direct merges only from `release/*` or hotfix branches
- **Rule:** Never commit directly, only merge pull requests

### `develop` (Integration)
- Integration branch for features
- All features branch from here
- **Rule:** Code should be tested before merging

### `feature/*` (Feature Development)
- Named: `feature/description` or `feature/ISSUE-123`
- Example: `feature/markdown-editor`, `feature/tauri-integration`
- Branch from: `develop`
- Merge back to: `develop` via pull request

### `bugfix/*` (Bug Fixes)
- Named: `bugfix/description`
- Example: `bugfix/folder-filtering`
- Branch from: `develop`
- Merge back to: `develop` via pull request

### `hotfix/*` (Emergency Production Fixes)
- Named: `hotfix/description`
- Example: `hotfix/database-corruption`
- Branch from: `main`
- Merge back to: `main` AND `develop`

## Common Workflows

### Starting a New Feature
```bash
# Update develop with latest changes
git checkout develop
git pull origin develop

# Create and switch to feature branch
git checkout -b feature/my-feature-name

# Make commits
git add .
git commit -m "Add new feature"

# Push to remote
git push -u origin feature/my-feature-name
```

### Creating a Pull Request
1. Push your branch
2. Go to GitHub/GitLab
3. Create PR from `feature/...` → `develop`
4. Add description
5. Request review
6. Merge when approved

### Updating Your Branch with Latest Changes
```bash
# Fetch latest
git fetch origin

# Rebase on develop (preferred) or merge
git rebase origin/develop
# or
git merge origin/develop

# Push updated branch
git push origin feature/my-feature-name --force-with-lease
```

### Releasing to Production
```bash
# Create release branch from develop
git checkout -b release/v1.0.0 develop

# Make version bumps, changelog updates
git commit -m "Bump version to v1.0.0"

# Merge to main
git checkout main
git merge --no-ff release/v1.0.0

# Tag the release
git tag -a v1.0.0 -m "Release v1.0.0"

# Merge back to develop
git checkout develop
git merge --no-ff release/v1.0.0

# Delete release branch
git branch -d release/v1.0.0
```

## Commit Message Convention

```
type(scope): subject

body

footer
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`

Examples:
```
feat(editor): add markdown keyboard shortcuts
fix(folder): resolve filtering issue
docs(readme): update installation instructions
chore(deps): update dependencies
```

## Local Setup

```bash
# Clone repository
git clone <repo-url>
cd Beck

# Set up local user (optional, if not already configured)
git config user.name "Your Name"
git config user.email "your@email.com"

# Checkout develop for feature work
git checkout develop
git pull origin develop
```

## Useful Commands

```bash
# List all branches (local and remote)
git branch -a

# Delete a local branch
git branch -d feature/old-feature

# Delete a remote branch
git push origin --delete feature/old-feature

# Rename current branch
git branch -m new-branch-name

# Show branch history
git log --oneline --graph --all

# See what changed in your branch
git diff develop...feature/my-feature
```
