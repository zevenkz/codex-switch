# Codex Switch UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a front-end-only `Codex Switch` preview that matches the repository's existing visual style and provides account cards plus a minimal settings page.

**Architecture:** Replace the current broad app shell with a focused two-view React shell driven by local state. Use mock account data, the existing i18n and theme-provider setup, and shadcn-style primitives so the preview feels native to the current codebase while avoiding backend dependencies.

**Tech Stack:** React, TypeScript, Vite, Tailwind CSS, Framer Motion, react-i18next, shadcn/ui, Vitest, Testing Library

---

## File Map

- Create: `src/features/codex-switch/mockAccounts.ts`
- Create: `src/features/codex-switch/types.ts`
- Create: `src/features/codex-switch/components/CodexSwitchSidebar.tsx`
- Create: `src/features/codex-switch/components/CodexAccountCard.tsx`
- Create: `src/features/codex-switch/components/CodexAccountsView.tsx`
- Create: `src/features/codex-switch/components/CodexSettingsView.tsx`
- Create: `src/features/codex-switch/CodexSwitchApp.tsx`
- Modify: `src/App.tsx`
- Modify: `src/i18n/locales/en.json`
- Modify: `src/i18n/locales/zh.json`
- Modify: `src/i18n/locales/ja.json`
- Modify: `tests/integration/App.test.tsx`

### Task 1: Define Mock Data And UI Types

**Files:**
- Create: `src/features/codex-switch/types.ts`
- Create: `src/features/codex-switch/mockAccounts.ts`
- Test: `tests/integration/App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("renders mock codex accounts on the accounts view", async () => {
  render(<App />);

  expect(await screen.findByText("amy@openai.dev")).toBeInTheDocument();
  expect(screen.getByText("Plus")).toBeInTheDocument();
  expect(screen.getByText("Enterprise")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "renders mock codex accounts on the accounts view"`
Expected: FAIL because the current app does not render the new Codex Switch preview or those mock account values.

- [ ] **Step 3: Write minimal implementation**

```ts
export type CodexAccountStatus = "active" | "available" | "needs_login";

export interface CodexAccount {
  id: string;
  email: string;
  plan: "Plus" | "Team" | "Enterprise";
  status: CodexAccountStatus;
  avatarSeed: string;
  isPrimary?: boolean;
}
```

```ts
import type { CodexAccount } from "./types";

export const mockAccounts: CodexAccount[] = [
  {
    id: "acct-1",
    email: "amy@openai.dev",
    plan: "Plus",
    status: "active",
    avatarSeed: "A",
    isPrimary: true,
  },
  {
    id: "acct-2",
    email: "research@openai.dev",
    plan: "Enterprise",
    status: "available",
    avatarSeed: "R",
  },
];
```

- [ ] **Step 4: Run test to verify it still fails for the right reason**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "renders mock codex accounts on the accounts view"`
Expected: FAIL because the app shell still has not been switched to render the new UI.

- [ ] **Step 5: Commit**

```bash
git add src/features/codex-switch/types.ts src/features/codex-switch/mockAccounts.ts tests/integration/App.test.tsx
git commit -m "test: define codex switch mock account fixtures"
```

### Task 2: Build The Focused App Shell And Accounts View

**Files:**
- Create: `src/features/codex-switch/components/CodexSwitchSidebar.tsx`
- Create: `src/features/codex-switch/components/CodexAccountCard.tsx`
- Create: `src/features/codex-switch/components/CodexAccountsView.tsx`
- Create: `src/features/codex-switch/CodexSwitchApp.tsx`
- Modify: `src/App.tsx`
- Test: `tests/integration/App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("shows the codex switch accounts layout and openai add-account link", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Switch")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /add account/i })).toBeInTheDocument();
  expect(screen.getByText("Accounts")).toBeInTheDocument();
  expect(screen.getByText("Settings")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "shows the codex switch accounts layout and openai add-account link"`
Expected: FAIL because the current `App` still renders the existing multi-feature product shell.

- [ ] **Step 3: Write minimal implementation**

```tsx
export function CodexSwitchApp() {
  const [view, setView] = useState<"accounts" | "settings">("accounts");

  return (
    <div className="flex min-h-screen bg-background text-foreground">
      <CodexSwitchSidebar view={view} onViewChange={setView} />
      {view === "accounts" ? <CodexAccountsView /> : <CodexSettingsView />}
    </div>
  );
}
```

```tsx
export default function App() {
  return <CodexSwitchApp />;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "shows the codex switch accounts layout and openai add-account link"`
Expected: PASS, and the accounts view renders the focused shell with the add-account action.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/features/codex-switch/components/CodexSwitchSidebar.tsx src/features/codex-switch/components/CodexAccountCard.tsx src/features/codex-switch/components/CodexAccountsView.tsx src/features/codex-switch/CodexSwitchApp.tsx tests/integration/App.test.tsx
git commit -m "feat: add codex switch accounts preview shell"
```

### Task 3: Build The Minimal Settings View

**Files:**
- Create: `src/features/codex-switch/components/CodexSettingsView.tsx`
- Modify: `tests/integration/App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("switches to settings and renders language and theme controls", async () => {
  render(<App />);

  await userEvent.click(screen.getByRole("button", { name: /settings/i }));

  expect(screen.getByText(/language/i)).toBeInTheDocument();
  expect(screen.getByText(/theme/i)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /english/i })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /dark/i })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "switches to settings and renders language and theme controls"`
Expected: FAIL because the new settings view does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```tsx
export function CodexSettingsView() {
  return (
    <div className="flex-1 p-6">
      <LanguageSettings value={language} onChange={handleLanguageChange} />
      <ThemeSettings />
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "switches to settings and renders language and theme controls"`
Expected: PASS and the settings view exposes only language and theme controls.

- [ ] **Step 5: Commit**

```bash
git add src/features/codex-switch/components/CodexSettingsView.tsx tests/integration/App.test.tsx
git commit -m "feat: add codex switch settings preview"
```

### Task 4: Add Preview Copy And Localization Keys

**Files:**
- Modify: `src/i18n/locales/en.json`
- Modify: `src/i18n/locales/zh.json`
- Modify: `src/i18n/locales/ja.json`
- Test: `tests/integration/App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("renders localized codex switch copy", async () => {
  render(<App />);

  expect(await screen.findByText(/switch between your codex accounts/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "renders localized codex switch copy"`
Expected: FAIL because the copy has not been added to locale files or the new view yet.

- [ ] **Step 3: Write minimal implementation**

```json
{
  "codexSwitch": {
    "title": "Codex Switch",
    "subtitle": "Switch between your Codex accounts with a focused macOS-style workspace.",
    "accounts": "Accounts",
    "settings": "Settings",
    "addAccount": "Add account"
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "renders localized codex switch copy"`
Expected: PASS and the new screen reads copy from i18n resources.

- [ ] **Step 5: Commit**

```bash
git add src/i18n/locales/en.json src/i18n/locales/zh.json src/i18n/locales/ja.json tests/integration/App.test.tsx
git commit -m "feat: add codex switch localized copy"
```

### Task 5: Verify The Full Preview Flow

**Files:**
- Modify: `tests/integration/App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("opens the add-account link and keeps the preview navigation working", async () => {
  const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);

  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: /add account/i }));
  await userEvent.click(screen.getByRole("button", { name: /settings/i }));
  await userEvent.click(screen.getByRole("button", { name: /accounts/i }));

  expect(openSpy).toHaveBeenCalledWith(
    "https://auth.openai.com/log-in",
    "_blank",
    "noopener,noreferrer",
  );
  expect(screen.getByText("amy@openai.dev")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run tests/integration/App.test.tsx -t "opens the add-account link and keeps the preview navigation working"`
Expected: FAIL until the add-account handler and navigation wiring are fully in place.

- [ ] **Step 3: Write minimal implementation**

```tsx
const handleAddAccount = () => {
  window.open("https://auth.openai.com/log-in", "_blank", "noopener,noreferrer");
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run tests/integration/App.test.tsx`
Expected: PASS for the full integration file with the new Codex Switch preview behavior.

- [ ] **Step 5: Commit**

```bash
git add tests/integration/App.test.tsx src/features/codex-switch/components/CodexAccountsView.tsx src/features/codex-switch/CodexSwitchApp.tsx
git commit -m "test: verify codex switch preview flow"
```
