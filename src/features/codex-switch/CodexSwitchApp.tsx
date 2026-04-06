import { useState } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { CodexAccountsView } from "./components/CodexAccountsView";
import { CodexSettingsView } from "./components/CodexSettingsView";
import { useCodexAccounts } from "./hooks/useCodexAccounts";

type CodexSwitchView = "accounts" | "settings";

export function CodexSwitchApp() {
  const [currentView, setCurrentView] = useState<CodexSwitchView>("accounts");
  const {
    accounts,
    isRefreshing,
    addAccount,
    refreshAll,
    switchAccount,
    deleteAccount,
  } = useCodexAccounts();
  const prefersReducedMotion = useReducedMotion();

  const mainAnimation = prefersReducedMotion
    ? {
        initial: false as const,
        animate: { opacity: 1, y: 0 },
        transition: { duration: 0 },
      }
    : {
        initial: { opacity: 0, y: 8 },
        animate: { opacity: 1, y: 0 },
        transition: { duration: 0.18, ease: "easeOut" as const },
      };

  return (
    <div className="min-h-screen bg-[#f5f7fb] px-6 py-6 text-slate-950 dark:bg-[#1f1f22] dark:text-slate-50">
      <div className="mx-auto flex min-h-[calc(100vh-3rem)] max-w-7xl flex-col">
        <motion.main
          initial={mainAnimation.initial}
          animate={mainAnimation.animate}
          transition={mainAnimation.transition}
          className="flex flex-1"
        >
          {currentView === "accounts" ? (
            <CodexAccountsView
              accounts={accounts}
              isRefreshing={isRefreshing}
              onAddAccount={() => void addAccount()}
              onRefreshQuota={() => void refreshAll()}
              onOpenSettings={() => setCurrentView("settings")}
              onSwitchAccount={(accountId) => void switchAccount(accountId)}
              onDeleteAccount={(accountId) => void deleteAccount(accountId)}
            />
          ) : (
            <CodexSettingsView onBack={() => setCurrentView("accounts")} />
          )}
        </motion.main>
      </div>
    </div>
  );
}

export default CodexSwitchApp;
