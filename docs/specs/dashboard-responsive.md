# Dashboard: responsive pass (Nov 2026)

Scope: `clients/web` (the embedded Nuxt dashboard). Audited in the browser at 375, 768 and 1280 px against the ux-guidelines rules for layout, touch, navigation and forms.

## What the audit found

No page scrolls sideways at 375 px, and the tables already sit in `overflow-x-auto`. The problems are in the shell and in touch ergonomics:

| # | Where | Problem |
|---|---|---|
| 1 | Top bar, phone | Project picker, mode pill, search, jobs, connection, theme and help all share 343 px: the mode pill is squeezed to "R", icons overlap |
| 2 | Bottom nav, phone | Six items (Home, Brain, Agents, Inbox, Safety, More); the guideline is five. "Sync & devices" is in no menu at all on a phone |
| 3 | Sidebar, tablet | At 768 px the 240 px sidebar takes a third of the screen; page titles and their buttons are squeezed side by side |
| 4 | Demo banner, phone | A long sentence truncated to "You are…" plus two buttons uses two lines of prime space |
| 5 | Buttons | Labels wrap ("+ Add note" on two lines); small buttons are about 28 px high, under the 44 px touch size |
| 6 | Tabs | The last tab is cut off with no hint that the row scrolls |
| 7 | Dialogs | Centred cards with 24 px padding on a phone; a bottom sheet is easier to reach and dismiss |
| 8 | Viewport | `h-screen` ignores the phone browser's moving toolbar; the bottom nav ignores the home-indicator safe area |

## Rules for the fix

- Breakpoints: under 640 phone (bottom nav, compact top bar), 640–1279 tablet (icon rail, 64 px), 1280 and up desktop (full sidebar, user can collapse).
- Bottom nav: five slots, Home, Brain, Agents, Inbox, More. Safety, Sync and the advanced pages live in More, with the current safety mode shown there. Help and theme are in More on a phone.
- Touch: 44 px targets on phones (40 px for compact buttons), labels never wrap in buttons.
- Everything keeps working with the keyboard and reduced motion; focus rings are unchanged.

## Not in this pass

New pages, new colours or fonts, and the Planning rooms layout (it already collapses to one column).
