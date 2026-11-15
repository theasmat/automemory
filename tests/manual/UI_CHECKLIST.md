# AutoMemory Dashboard UI / UX Manual QA Checklist

This checklist verifies the dashboard across responsive breakpoints, theme modes, accessibility standards, and runtime edge cases (as specified in sections 10 and 12 of `docs/specs/dashboard-ui.md`).

---

## 1. Theme and Appearance (Dark / Light / System)

- [ ] **Default Appearance on First Visit**:
  - Clear `localStorage.removeItem('am_theme')`.
  - Reload `http://127.0.0.1:4551/`.
  - Verify that theme follows system preference (`window.matchMedia('(prefers-color-scheme: dark)')`) with 0ms flash of incorrect color.
  - Check that native scrollbars and controls match `color-scheme`.
- [ ] **TopBar Theme Switcher Cycle**:
  - Click theme toggle pill in TopBar.
  - Verify it cleanly cycles: `System` -> `Light` -> `Dark` -> `System`.
  - Icon and label update to match the active state.
  - Hover tooltip and `aria-label` reflect the current mode ("Theme: System", "Theme: Light", "Theme: Dark").
- [ ] **Switch Theme with a Modal Open**:
  - Trigger PasswordModal, AddNote modal, or Search/Command Palette.
  - Switch theme (e.g., via ⌘K command "Switch to Light Theme" or while modal is open).
  - Verify backdrop (`--overlay`) dims correctly, modal surface adapts smoothly, and inputs maintain proper focus rings.
- [ ] **System Theme Change While Page is Open**:
  - Set theme to `System`.
  - Toggle OS appearance between Dark and Light mode (macOS System Settings -> Appearance).
  - Verify dashboard updates live within 150ms without needing a page refresh.
- [ ] **Reload with Each Stored Value**:
  - Explicitly select `Dark` -> reload. Verify instant dark theme.
  - Explicitly select `Light` -> reload. Verify instant warm paper light theme.
  - Explicitly select `System` -> reload. Verify system theme match.
- [ ] **Forced Colors Mode**:
  - Simulate forced colors (`forced-colors: active` in browser rendering tools).
  - Verify controls, focus indicators, and 1px borders remain clearly visible with high contrast.
- [ ] **Print Mode**:
  - Open browser Print preview (⌘P).
  - Verify page renders in clean light tokens with `--bg: #ffffff`, no heavy box shadows, and crisp 1px borders.

---

## 2. Accessibility (WCAG 2.2 AA)

- [ ] **Color Contrast**:
  - Verify automated script passes: `python3 tests/ui/contrast.py`.
  - Check all pages in Light mode: no washed-out text, no low-contrast gold on white text (solid accent uses `#1f1500` on `#f2b100`).
- [ ] **Focus Rings**:
  - Tab through all interactive elements on Home, Brain, Agents, Inbox, Sync, Safety.
  - Verify prominent 2px outline with 2px offset on all buttons, links, and inputs.
- [ ] **Keyboard Navigation**:
  - `⌘K` / `Ctrl+K`: Opens Command Palette; arrow keys navigate, Enter selects, Esc dismisses.
  - `?`: Opens shortcut help modal.
  - `n`: Opens "+ Add note" modal.
  - `g h`, `g b`, `g a`, `g i`, `g s`: Vim jump shortcuts to respective views.
- [ ] **Screen Reader Live Regions**:
  - Toast notifications announce additions and archival actions (`role="status"`, `aria-live="polite"`).
  - Connection status changes announce daemon connectivity transitions.

---

## 3. Responsive Breakpoints

- [ ] **Mobile (< 768px, e.g., 360px wide)**:
  - Top bar renders cleanly with project picker, status dot, and theme toggle.
  - Bottom navigation tab bar displays with badges and a "More" drawer.
  - Horizontal tables stack gracefully into readable cards.
- [ ] **Tablet (768px - 1279px)**:
  - NavRail collapses to icon-only mode with tooltips.
  - Content flows into a balanced two-column layout.
- [ ] **Desktop (>= 1280px)**:
  - NavRail expands to full width with icon labels.
  - Home dashboard renders 3-column view without horizontal overflow.

---

## 4. Workflows & States

- [ ] **First Run**:
  - When no agents or notes exist, empty states provide friendly plain-English instructions and one-click actions.
- [ ] **Offline Daemon**:
  - Stop `automemory serve`.
  - Top bar status dot switches to warning pulse ("Connecting...").
  - Content remains readable; offline notice displays `automemory serve` command to copy.
- [ ] **Agent Collision & Locks** (needs the Tracewise companion; see LIVE_CHECKLIST section 0):
  - Open `/agents` while an agent holds a lock (taken by Tracewise on its first edit).
  - Holder name, file and time left display; the empty state and the "How file locks work" text name Tracewise.
  - Without Tracewise installed the list is empty and says nothing is locked (no error).
  - Note: the compiled dashboard bundle still carries the old wording until the Nuxt app is rebuilt.
- [ ] **Protected Actions**:
  - Modifying guardrail rules, releasing a lock (asks Tracewise), or changing safety mode triggers the masked `PasswordModal` instead of an unmasked browser prompt.
