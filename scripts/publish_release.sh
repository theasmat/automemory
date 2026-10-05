#!/usr/bin/env bash
# ==============================================================================
# AutoMemory Release & Distribution Automation
# https://github.com/theasmat/automemory
#
# Usage:
#   ./scripts/publish_release.sh [version_tag]
# Example:
#   ./scripts/publish_release.sh v0.1.1
# ==============================================================================

set -euo pipefail

# ANSI color codes
BOLD="$(tput bold 2>/dev/null || printf '')"
GREEN="$(tput setaf 2 2>/dev/null || printf '')"
YELLOW="$(tput setaf 3 2>/dev/null || printf '')"
CYAN="$(tput setaf 6 2>/dev/null || printf '')"
RED="$(tput setaf 1 2>/dev/null || printf '')"
RESET="$(tput sgr0 2>/dev/null || printf '')"

log_info() {
    printf "${CYAN}==>${RESET} ${BOLD}%s${RESET}\n" "$1"
}

log_success() {
    printf "${GREEN}✓${RESET} ${BOLD}%s${RESET}\n" "$1"
}

log_warn() {
    printf "${YELLOW}⚠${RESET} %s\n" "$1"
}

log_error() {
    printf "${RED}✗ Error:${RESET} %s\n" "$1" >&2
}

SRC_REPO="theasmat/automemory"
DIST_REPO="theasmat/automemory-bin"

# 1. Determine Tag
TAG="${1:-}"
if [ -z "$TAG" ]; then
    printf "${CYAN}Enter release tag (e.g. v0.1.1): ${RESET}"
    read -r TAG
fi

if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+.*$ ]]; then
    log_error "Tag must follow semantic versioning format (e.g. v0.1.1)"
    exit 1
fi

log_info "Preparing automated release for: ${BOLD}${TAG}${RESET}"

# 2. Check Prerequisites
if ! command -v gh >/dev/null 2>&1; then
    log_error "GitHub CLI (gh) is required. Install via 'brew install gh'"
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    log_error "Rust/Cargo is required to build the release binary."
    exit 1
fi

# 3. Compile Release Profile
log_info "Compiling optimized release binary..."
cargo build --release

# 4. Verify Test Suite
log_info "Running workspace verification tests..."
cargo test --release --workspace

# 5. Package Binary Archive
TARGET_TRIPLE="aarch64-apple-darwin"
ARCHIVE_NAME="automemory-${TARGET_TRIPLE}.tar.gz"
CHECKSUM_NAME="${ARCHIVE_NAME}.sha256"
TEMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'automemory-release')"
trap 'rm -rf "${TEMP_DIR}"' EXIT

log_info "Packaging standalone release archive (${ARCHIVE_NAME})..."
tar -czf "${TEMP_DIR}/${ARCHIVE_NAME}" -C target/release automemory
(cd "${TEMP_DIR}" && shasum -a 256 "${ARCHIVE_NAME}" > "${CHECKSUM_NAME}")

log_success "Created release package: $(ls -lh "${TEMP_DIR}/${ARCHIVE_NAME}" | awk '{print $5}')"

# 6. Synchronize Public Distribution Repo (automemory-bin)
SYNC_DIR="${TEMP_DIR}/automemory-bin"
log_info "Syncing public distribution repository (${DIST_REPO})..."
git clone "https://github.com/${DIST_REPO}.git" "${SYNC_DIR}"

# Copy latest installer scripts
cp install.sh "${SYNC_DIR}/install.sh"
cp install.ps1 "${SYNC_DIR}/install.ps1"
chmod +x "${SYNC_DIR}/install.sh"

(
    cd "${SYNC_DIR}"
    if [ -n "$(git status --porcelain)" ]; then
        git add -A
        git commit -m "chore: sync universal installers for ${TAG}"
        git push origin master
        git push origin master:main || true
        log_success "Pushed updated installer scripts to ${DIST_REPO}"
    else
        log_info "Distribution repo installer scripts are already up-to-date."
    fi
)

# 7. Publish GitHub Release on the Public Distribution Repo
log_info "Publishing release ${TAG} on ${DIST_REPO}..."
gh release create "${TAG}" \
    "${TEMP_DIR}/${ARCHIVE_NAME}" \
    "${TEMP_DIR}/${CHECKSUM_NAME}" \
    --repo "${DIST_REPO}" \
    --title "AutoMemory ${TAG}" \
    --generate-notes || {
        log_warn "Release ${TAG} may already exist, uploading assets..."
        gh release upload "${TAG}" \
            "${TEMP_DIR}/${ARCHIVE_NAME}" \
            "${TEMP_DIR}/${CHECKSUM_NAME}" \
            --repo "${DIST_REPO}" \
            --clobber
    }

log_success "Published release to public repository: https://github.com/${DIST_REPO}/releases/tag/${TAG}"

# 8. Tag the Private Source Repository
log_info "Tagging source repository (${SRC_REPO})..."
if git rev-parse "${TAG}" >/dev/null 2>&1; then
    log_info "Git tag ${TAG} already exists locally."
else
    git tag -a "${TAG}" -m "Release ${TAG}"
    git push origin "${TAG}" || log_warn "Could not push tag to private repo (it may already exist)."
    log_success "Pushed tag ${TAG} to ${SRC_REPO}"
fi

printf "\n"
printf "${BOLD}${GREEN}========================================================${RESET}\n"
printf "  ${BOLD}${GREEN}AutoMemory Release ${TAG} Published Successfully!${RESET}     \n"
printf "${BOLD}${GREEN}========================================================${RESET}\n\n"
printf "Public Download URL:\n"
printf "  ${CYAN}https://github.com/%s/releases/tag/%s${RESET}\n\n" "${DIST_REPO}" "${TAG}"
printf "Public One-Liner Installers for Testers:\n"
printf "  ${BOLD}macOS / Linux:${RESET}\n"
printf "    ${YELLOW}curl -fsSL https://raw.githubusercontent.com/%s/main/install.sh | bash${RESET}\n\n" "${DIST_REPO}"
printf "  ${BOLD}Windows (PowerShell):${RESET}\n"
printf "    ${YELLOW}irm https://raw.githubusercontent.com/%s/main/install.ps1 | iex${RESET}\n\n" "${DIST_REPO}"
