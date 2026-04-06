# Codex Switch UI Preview Design

## Overview

This spec defines a front-end-only `Codex Switch` preview built inside the current repository. The goal is to reuse the existing project's visual language and component style while simplifying the information architecture into a focused macOS account-switching prototype.

The preview will not include any backend integration, persistence, or real authentication. All account content is mock data, and the "add account" flow is a UI-only jump to the OpenAI login page.

## Goals

- Follow the current repository's React, TypeScript, Tailwind, shadcn/ui, Framer Motion, i18n, and theme-provider patterns.
- Match the existing visual feel: glass cards, soft borders, blue primary actions, compact desktop spacing, and restrained motion.
- Build a focused `Codex Switch` experience with only two functional areas:
  - account display and add-account entry
  - settings for language and theme
- Use mock account data so the user can preview the UI before any backend work begins.

## Non-Goals

- No Tauri command calls
- No real OpenAI or Codex authentication
- No local persistence or account storage
- No menu bar, tray, native macOS automation, or account switching backend
- No account nickname, no recent usage time in cards

## App Structure

The preview will replace the current front-end shell with a lighter application layout tailored to `Codex Switch`.

### Navigation

- A compact left navigation rail with two items:
  - `Accounts`
  - `Settings`
- The rail keeps the desktop-tool feel while reducing the current app's broader multi-feature navigation.

### Main Content

- The right side is the active content area.
- The default view is `Accounts`.
- A lightweight top header shows the product name, a short subtitle, and context-aware actions.

## Accounts View

The accounts page is the primary preview surface and uses a card list layout.

### Header Area

- Product title: `Codex Switch`
- Short explanatory subtitle
- Primary CTA: `Add account`
- The CTA visually matches the project's existing primary action styling

### Account Cards

Each mock account card will include:

- avatar or generated initial badge
- email address
- plan or identity badge, such as `Plus`, `Team`, or `Enterprise`
- current status, such as `Active`, `Available`, or `Needs login`
- a compact action row for preview purposes

Each card will intentionally exclude:

- account display name
- recent usage time

### Add Account Flow

- Clicking `Add account` opens the OpenAI login page in a new browser tab or window.
- The UI will frame this as the first step of connecting a Codex account.
- No callback handling or session capture is included in this preview.

### Mock Data

The page will ship with multiple example accounts to show variety across:

- avatar colors
- plan badges
- status indicators
- active/default visual state

## Settings View

The settings page is intentionally minimal and mirrors the existing repository's settings implementation style.

### Language Section

- segmented button group
- options:
  - Chinese
  - English
  - Japanese

### Theme Section

- segmented button group with icons
- options:
  - Light
  - Dark
  - System

### Behavior

- Language selection updates front-end UI text
- Theme selection uses the existing theme provider
- No save button is required for this preview if the interaction can stay immediate

## Visual Style

The preview should directly borrow from existing project styling rather than inventing a new design system.

### Reused Style Principles

- existing CSS variables from `src/index.css`
- glass and glass-card surfaces
- rounded corners and subtle borders
- blue primary buttons
- muted text hierarchy
- restrained Framer Motion transitions

### Adaptation Rules

- simplify layout density for a focused product
- keep spacing and radius consistent with existing components
- use the same button, card, badge, dialog, and input primitives where possible
- avoid introducing a new color system or typography direction

## Component Boundaries

The preview should be organized into focused React components.

### Expected Units

- app shell component
- navigation component
- accounts page component
- account card component
- settings page component
- small local mock data module

This keeps the preview easy to extend later with real account and auth logic.

## Data Flow

- Local React state controls current page selection
- Mock account data is imported from a front-end module
- Theme state continues to flow through the existing theme provider
- Language state should follow the current i18n setup

## Testing

The implementation should include lightweight front-end verification for:

- accounts page rendering mock cards
- add-account button linking to the OpenAI login page
- settings page rendering language and theme controls
- navigation switching between `Accounts` and `Settings`

## Risks And Constraints

- Replacing the existing app shell for preview purposes may temporarily hide unrelated product areas; that is acceptable for this branch because the user asked for a focused `Codex Switch` preview.
- The OpenAI login entry is only a visual preview, so the UI must not imply that account connection already works.
- The page should stay believable as a macOS desktop utility and not drift into a generic dashboard look.
