import { useRef, type KeyboardEvent } from "react";
import { Settings, UsersRound } from "lucide-react";
import { cn } from "@/lib/utils";
import { useTranslation } from "react-i18next";

export type CodexSwitchView = "accounts" | "settings";

interface CodexSwitchSidebarProps {
  currentView: CodexSwitchView;
  onViewChange: (view: CodexSwitchView) => void;
}

const panelIds: Record<CodexSwitchView, string> = {
  accounts: "accounts-panel",
  settings: "settings-panel",
};

const sidebarItems = [
  {
    id: "accounts" as const,
    icon: UsersRound,
    labelKey: "codexSwitch.nav.accounts",
  },
  {
    id: "settings" as const,
    icon: Settings,
    labelKey: "codexSwitch.nav.settings",
  },
];

const viewOrder = sidebarItems.map((item) => item.id);

export function CodexSwitchSidebar({
  currentView,
  onViewChange,
}: CodexSwitchSidebarProps) {
  const { t } = useTranslation();
  const tabRefs = useRef<
    Partial<Record<CodexSwitchView, HTMLButtonElement | null>>
  >({});

  const focusView = (view: CodexSwitchView) => {
    tabRefs.current[view]?.focus();
    onViewChange(view);
  };

  const handleKeyDown = (
    event: KeyboardEvent<HTMLButtonElement>,
    view: CodexSwitchView,
  ) => {
    const currentIndex = viewOrder.indexOf(view);

    if (currentIndex === -1) {
      return;
    }

    let nextView: CodexSwitchView | null = null;

    switch (event.key) {
      case "ArrowDown":
        nextView = viewOrder[(currentIndex + 1) % viewOrder.length];
        break;
      case "ArrowUp":
        nextView =
          viewOrder[(currentIndex - 1 + viewOrder.length) % viewOrder.length];
        break;
      case "Home":
        nextView = viewOrder[0];
        break;
      case "End":
        nextView = viewOrder[viewOrder.length - 1];
        break;
      default:
        return;
    }

    event.preventDefault();
    focusView(nextView);
  };

  return (
    <aside className="glass-card flex w-full max-w-[220px] flex-col rounded-[28px] border border-white/60 p-3 shadow-[0_24px_80px_rgba(15,23,42,0.12)] dark:border-white/10 dark:shadow-[0_24px_80px_rgba(2,6,23,0.4)]">
      <div className="mb-5 px-3 pt-2">
        <p className="text-xs font-semibold uppercase tracking-[0.24em] text-slate-400 dark:text-slate-500">
          {t("codexSwitch.nav.workspace")}
        </p>
      </div>
      <div
        className="flex flex-col gap-1.5"
        role="tablist"
        aria-label="Primary"
        aria-orientation="vertical"
      >
        {sidebarItems.map(({ id, icon: Icon, labelKey }) => {
          const isActive = currentView === id;
          const label = t(labelKey);

          return (
            <button
              key={id}
              id={`${id}-tab`}
              ref={(element) => {
                tabRefs.current[id] = element;
              }}
              type="button"
              role="tab"
              aria-selected={isActive}
              aria-controls={panelIds[id]}
              tabIndex={isActive ? 0 : -1}
              className={cn(
                "flex items-center gap-3 rounded-2xl px-3 py-3 text-left text-sm font-medium transition-colors",
                isActive
                  ? "bg-blue-500 text-white shadow-[0_16px_32px_rgba(59,130,246,0.24)]"
                  : "text-slate-600 hover:bg-white/70 hover:text-slate-900 dark:text-slate-300 dark:hover:bg-white/5 dark:hover:text-white",
              )}
              onClick={() => onViewChange(id)}
              onKeyDown={(event) => handleKeyDown(event, id)}
            >
              <Icon className="h-4 w-4" />
              <span>{label}</span>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
