import { Suspense, type ComponentType } from "react";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, it, expect, vi } from "vitest";
import { ThemeProvider } from "@/components/theme-provider";
import { mockAccounts } from "@/features/codex-switch/mockAccounts";
import { resolveSupportedLanguage } from "@/features/codex-switch/components/CodexSettingsView";
import i18n from "@/i18n";

type CodexQuotaSnapshot = {
  five_hour_percent: number | null;
  five_hour_reset_at: number | null;
  week_percent: number | null;
  week_reset_at: number | null;
  refreshed_at: number;
  last_error: string | null;
};

type CodexAccountRecord = {
  id: string;
  email: string | null;
  account_id: string | null;
  plan_type: string | null;
  display_name: string | null;
  avatar_seed: string;
  added_at: number;
  last_used_at: number | null;
  is_active: boolean;
  auth_json: Record<string, unknown>;
  quota: CodexQuotaSnapshot | null;
  metadata: Record<string, unknown>;
};

type PendingCodexOAuthSession = {
  state: string;
  authorize_url: string;
  callback_port: number;
};

const { invokeMock, listenMock } = vi.hoisted(() => {
  const invokeMock = vi.fn();
  const listenMock = vi.fn();
  return { invokeMock, listenMock };
});

let accountsUpdatedHandler:
  | ((event: { payload: unknown }) => void | Promise<void>)
  | null = null;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

const renderApp = (AppComponent: ComponentType) =>
  render(
    <ThemeProvider defaultTheme="system" storageKey="codex-switch-theme-test">
      <Suspense fallback={<div data-testid="loading">loading</div>}>
        <AppComponent />
      </Suspense>
    </ThemeProvider>,
  );

const t = (key: string, options?: Record<string, unknown>) =>
  i18n.t(key, options);

function makeAccountRecord(
  overrides: Partial<CodexAccountRecord> & Pick<CodexAccountRecord, "id" | "email" | "account_id" | "is_active">,
): CodexAccountRecord {
  return {
    id: overrides.id,
    email: overrides.email,
    account_id: overrides.account_id,
    plan_type: overrides.plan_type ?? "plus",
    display_name: overrides.display_name ?? overrides.email ?? overrides.id,
    avatar_seed: overrides.avatar_seed ?? overrides.email?.slice(0, 1).toUpperCase() ?? "C",
    added_at: overrides.added_at ?? 1,
    last_used_at: overrides.last_used_at ?? 2,
    is_active: overrides.is_active,
    auth_json: overrides.auth_json ?? {},
    quota:
      overrides.quota ??
      ({
        five_hour_percent: 35,
        five_hour_reset_at: 1712919420,
        week_percent: 38,
        week_reset_at: 1712923020,
        refreshed_at: 1712915820,
        last_error: null,
      } satisfies CodexQuotaSnapshot),
    metadata: overrides.metadata ?? {},
  };
}

describe("Codex Switch app shell", () => {
  beforeEach(async () => {
    window.localStorage.clear();
    document.documentElement.classList.remove("light", "dark");
    await i18n.changeLanguage("zh");

    invokeMock.mockReset();
    listenMock.mockReset();
    accountsUpdatedHandler = null;

    listenMock.mockImplementation(async (eventName, handler) => {
      if (eventName === "codex-accounts-updated") {
        accountsUpdatedHandler = handler as typeof accountsUpdatedHandler;
      }

      return () => {};
    });
  });

  it(
    "renders accounts from Tauri commands, auto-refreshes on load, and refreshes when the update event fires",
    async () => {
    const oauthSession: PendingCodexOAuthSession = {
      state: "state-1",
      authorize_url: "https://auth.openai.com/oauth/authorize?test=1",
      callback_port: 1455,
    };

    let accounts: CodexAccountRecord[] = [
      makeAccountRecord({
        id: "acct-1",
        email: "amy@openai.dev",
        account_id: "acct-1",
        is_active: true,
        plan_type: "plus",
        display_name: "Amy",
        avatar_seed: "A",
      }),
      makeAccountRecord({
        id: "acct-2",
        email: "research@openai.dev",
        account_id: "acct-2",
        is_active: false,
        plan_type: "enterprise",
        display_name: "Research",
        avatar_seed: "R",
        quota: {
          five_hour_percent: 62,
          five_hour_reset_at: 1712923020,
          week_percent: 71,
          week_reset_at: 1713009420,
          refreshed_at: 1712915820,
          last_error: null,
        },
      }),
    ];

    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "list_codex_accounts":
          return JSON.parse(JSON.stringify(accounts));
        case "start_codex_account_oauth":
          return oauthSession;
        case "complete_codex_account_oauth":
          accounts = [
            ...accounts,
            makeAccountRecord({
              id: "acct-3",
              email: "new-user@openai.dev",
              account_id: "acct-3",
              is_active: false,
              plan_type: "team",
              display_name: "New User",
              avatar_seed: "N",
              quota: {
                five_hour_percent: 44,
                five_hour_reset_at: 1712926620,
                week_percent: 57,
                week_reset_at: 1713095820,
                refreshed_at: 1712915820,
                last_error: null,
              },
            }),
          ];
          return JSON.parse(JSON.stringify(accounts[accounts.length - 1]));
        case "open_external":
          return true;
        case "set_window_theme":
          return true;
        case "refresh_all_codex_account_quotas":
          accounts = accounts.map((account) => ({
            ...account,
            quota: account.quota
              ? {
                  ...account.quota,
                  five_hour_percent: account.id === "acct-1" ? 20 : 40,
                  week_percent: account.id === "acct-1" ? 50 : 60,
                }
              : account.quota,
          }));
          return JSON.parse(JSON.stringify(accounts));
        case "switch_codex_account":
          accounts = accounts.map((account) => ({
            ...account,
            is_active:
              account.account_id === args?.accountId || account.id === args?.accountId,
          }));
          return true;
        case "delete_codex_account":
          accounts = accounts.filter(
            (account) => account.account_id !== args?.accountId && account.id !== args?.accountId,
          );
          return true;
        default:
          return undefined;
      }
    });

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    expect(screen.getByRole("heading", { name: "Codex Switch" })).toBeInTheDocument();
    expect(
      await screen.findByText(
        t("codexSwitch.accounts.countSummary", { count: accounts.length }),
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("codexSwitch.accounts.addAccount") }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("codexSwitch.accounts.refreshQuota") }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("common.settings") }),
    ).toBeInTheDocument();
    for (const account of accounts) {
      expect(screen.getByText(account.email as string)).toBeInTheDocument();
    }
    expect(screen.getAllByText(t("codexSwitch.quota.fiveHours")).length).toBeGreaterThan(0);
    expect(screen.getAllByText(t("codexSwitch.quota.week")).length).toBeGreaterThan(0);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("refresh_all_codex_account_quotas"),
    );
    expect(screen.getAllByText("20%").length).toBeGreaterThan(0);
    expect(screen.getAllByText("50%").length).toBeGreaterThan(0);
    expect(screen.getByTestId("quota-panel-acct-1").className).toContain("md:w-[200px]");
    expect(screen.getByTestId("quota-panel-acct-2").className).toContain("md:w-[200px]");
    expect(screen.getByTestId("account-actions-acct-1").className).toContain("md:w-[132px]");
    expect(screen.getByTestId("account-actions-acct-2").className).toContain("md:w-[132px]");
    expect(
      screen.getByRole("button", { name: t("codexSwitch.accounts.inUse") }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("codexSwitch.accounts.enable") }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: t("codexSwitch.accounts.addAccount") }));
    expect(invokeMock).toHaveBeenCalledWith("start_codex_account_oauth");
    expect(invokeMock).toHaveBeenCalledWith("open_external", {
      url: oauthSession.authorize_url,
    });
    expect(invokeMock).toHaveBeenCalledWith("complete_codex_account_oauth");
    await waitFor(() =>
      expect(screen.getByText("new-user@openai.dev")).toBeInTheDocument(),
    );
    expect(
      screen.getByText(t("codexSwitch.accounts.countSummary", { count: 3 })),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: t("codexSwitch.accounts.refreshQuota") }),
    );

    await waitFor(() => expect(accountsUpdatedHandler).not.toBeNull());
    accounts = accounts.map((account) => ({
      ...account,
      quota: account.quota
        ? { ...account.quota, five_hour_percent: 77, week_percent: 88 }
        : account.quota,
    }));
    await accountsUpdatedHandler?.({ payload: null });

    await waitFor(() =>
      expect(screen.getAllByText("77%").length).toBeGreaterThan(0),
    );
    expect(screen.getAllByText("88%").length).toBeGreaterThan(0);
    },
    15_000,
  );

  it("opens settings from the header action and returns back to the main page", async () => {
    invokeMock.mockResolvedValue([
      makeAccountRecord({
        id: "acct-1",
        email: "amy@openai.dev",
        account_id: "acct-1",
        is_active: true,
      }),
    ]);

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    await user.click(screen.getByRole("button", { name: t("common.settings") }));

    expect(
      screen.getByRole("heading", { name: t("common.settings") }),
    ).toBeInTheDocument();
    expect(screen.getByText(t("settings.language"))).toBeInTheDocument();
    expect(screen.getByText(t("settings.theme"))).toBeInTheDocument();
    expect(
      screen.getByRole("radio", {
        name: t("settings.languageOptionEnglish"),
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("settings.themeSystem") }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("common.back") }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: t("common.back") }));

    expect(screen.getByRole("heading", { name: "Codex Switch" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: t("common.settings") }),
    ).toBeInTheDocument();
  });

  it("updates i18n language and persists it when language changes", async () => {
    invokeMock.mockResolvedValue([
      makeAccountRecord({
        id: "acct-1",
        email: "amy@openai.dev",
        account_id: "acct-1",
        is_active: true,
      }),
    ]);

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    await user.click(screen.getByRole("button", { name: t("common.settings") }));
    const englishRadio = screen.getByRole("radio", {
      name: t("settings.languageOptionEnglish"),
    });

    await user.click(englishRadio);

    expect(
      await screen.findByRole("heading", { name: t("common.settings") }),
    ).toBeInTheDocument();
    expect(await screen.findByText(t("settings.language"))).toBeInTheDocument();
    expect(await screen.findByText(t("settings.theme"))).toBeInTheDocument();
    expect(
      await screen.findByRole("radio", {
        name: t("settings.languageOptionEnglish"),
        checked: true,
      }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: t("common.back") })).toBeInTheDocument();
    expect(window.localStorage.getItem("language")).toBe("en");
  });

  it("normalizes locale variants to the matching supported language option", () => {
    expect(resolveSupportedLanguage("en-US")).toBe("en");
    expect(resolveSupportedLanguage("ja-JP")).toBe("ja");
    expect(resolveSupportedLanguage("zh-CN")).toBe("zh");
    expect(resolveSupportedLanguage("en")).toBe("en");
  });

  it("updates the active theme in an observable way", async () => {
    invokeMock.mockResolvedValue([
      makeAccountRecord({
        id: "acct-1",
        email: "amy@openai.dev",
        account_id: "acct-1",
        is_active: true,
      }),
    ]);

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    await user.click(screen.getByRole("button", { name: t("common.settings") }));
    await user.click(screen.getByRole("button", { name: t("settings.themeDark") }));

    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.classList.contains("light")).toBe(false);
    expect(window.localStorage.getItem("codex-switch-theme-test")).toBe("dark");
  });

  it("supports single-select keyboard navigation in the language control", async () => {
    invokeMock.mockResolvedValue([
      makeAccountRecord({
        id: "acct-1",
        email: "amy@openai.dev",
        account_id: "acct-1",
        is_active: true,
      }),
    ]);

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    await user.click(screen.getByRole("button", { name: t("common.settings") }));

    const languageGroup = screen.getByRole("radiogroup", {
      name: t("settings.language"),
    });
    const chineseRadio = within(languageGroup).getByRole("radio", {
      name: t("settings.languageOptionChinese"),
    });
    const englishRadio = within(languageGroup).getByRole("radio", {
      name: t("settings.languageOptionEnglish"),
    });
    const japaneseRadio = within(languageGroup).getByRole("radio", {
      name: t("settings.languageOptionJapanese"),
    });

    expect(chineseRadio).toHaveAttribute("aria-checked", "true");
    expect(chineseRadio).toHaveAttribute("tabindex", "0");
    expect(englishRadio).toHaveAttribute("aria-checked", "false");
    expect(englishRadio).toHaveAttribute("tabindex", "-1");

    chineseRadio.focus();
    expect(chineseRadio).toHaveFocus();

    await user.keyboard("{ArrowRight}");
    expect(englishRadio).toHaveFocus();
    expect(englishRadio).toHaveAttribute("aria-checked", "true");
    expect(window.localStorage.getItem("language")).toBe("en");

    await user.keyboard("{End}");
    expect(japaneseRadio).toHaveFocus();
    expect(japaneseRadio).toHaveAttribute("aria-checked", "true");
    expect(window.localStorage.getItem("language")).toBe("ja");

    await user.keyboard("{Home}");
    expect(chineseRadio).toHaveFocus();
    expect(chineseRadio).toHaveAttribute("aria-checked", "true");
    expect(window.localStorage.getItem("language")).toBe("zh");
  });

  it("returns from settings to the accounts page with the back action", async () => {
    invokeMock.mockResolvedValue([
      makeAccountRecord({
        id: "acct-1",
        email: "amy@openai.dev",
        account_id: "acct-1",
        is_active: true,
      }),
    ]);

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    await user.click(screen.getByRole("button", { name: t("common.settings") }));
    await user.click(screen.getByRole("button", { name: t("common.back") }));

    expect(screen.getByRole("heading", { name: "Codex Switch" })).toBeInTheDocument();
  });

  it("shows a delete action for inactive accounts and removes them after confirmation", async () => {
    let accounts: CodexAccountRecord[] = [
      makeAccountRecord({
        id: "acct-active",
        email: "active@example.com",
        account_id: "acct-active",
        is_active: true,
      }),
      makeAccountRecord({
        id: "acct-inactive",
        email: "inactive@example.com",
        account_id: "acct-inactive",
        is_active: false,
      }),
    ];

    invokeMock.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "list_codex_accounts":
          return JSON.parse(JSON.stringify(accounts));
        case "delete_codex_account":
          accounts = accounts.filter(
            (account) => account.account_id !== args?.accountId && account.id !== args?.accountId,
          );
          return true;
        case "set_window_theme":
          return true;
        default:
          return undefined;
      }
    });

    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    expect(
      screen.queryByRole("button", {
        name: `${t("common.delete")} active@example.com`,
      }),
    ).not.toBeInTheDocument();

    const deleteButton = await screen.findByRole("button", {
      name: `${t("common.delete")} inactive@example.com`,
    });

    await user.click(deleteButton);

    expect(
      screen.getByRole("heading", { name: t("codexSwitch.accounts.deleteTitle") }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        t("codexSwitch.accounts.deleteMessage", {
          email: "inactive@example.com",
        }),
      ),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: t("codexSwitch.accounts.deleteConfirm") }),
    );

    await waitFor(() =>
      expect(screen.queryByText("inactive@example.com")).not.toBeInTheDocument(),
    );
    expect(
      screen.getByText(t("codexSwitch.accounts.countSummary", { count: 1 })),
    ).toBeInTheDocument();
  });

  it("falls back to the mock preview when Tauri commands are unavailable", async () => {
    invokeMock.mockRejectedValue(new Error("no tauri runtime"));
    listenMock.mockRejectedValue(new Error("no event bridge"));

    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    const user = userEvent.setup();
    const { default: App } = await import("@/App");
    renderApp(App);

    for (const account of mockAccounts) {
      expect(await screen.findByText(account.email)).toBeInTheDocument();
    }

    await user.click(
      screen.getByRole("button", { name: t("codexSwitch.accounts.addAccount") }),
    );

    expect(openSpy).toHaveBeenCalled();
    openSpy.mockRestore();
  });

  it("does not render fake zero quotas when the backend reports quota refresh failures", async () => {
    invokeMock.mockResolvedValue([
      makeAccountRecord({
        id: "acct-1",
        email: "quota-error@example.com",
        account_id: "acct-1",
        is_active: true,
        quota: {
          five_hour_percent: null,
          five_hour_reset_at: null,
          week_percent: null,
          week_reset_at: null,
          refreshed_at: 1712915820,
          last_error: "usage fetch failed",
        },
      }),
    ]);

    const { default: App } = await import("@/App");
    renderApp(App);

    expect(await screen.findByText("quota-error@example.com")).toBeInTheDocument();
    expect(screen.queryByText("0%")).not.toBeInTheDocument();
    expect(screen.queryByText(t("codexSwitch.quota.fiveHours"))).not.toBeInTheDocument();
    expect(screen.queryByText(t("codexSwitch.quota.week"))).not.toBeInTheDocument();
    expect(
      screen.getByText(t("codexSwitch.quota.refreshFailed")),
    ).toBeInTheDocument();
  });
});
