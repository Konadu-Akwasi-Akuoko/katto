import { useState } from "react";
import { Providers } from "@/app/providers";
import { AppShell } from "@/components/layout/app-shell";
import { Sidebar } from "@/components/layout/sidebar";
import { Titlebar } from "@/components/layout/titlebar";
import { Toaster } from "@/components/ui/sonner";
import { Dashboard } from "@/features/dashboard/dashboard";

export default function App() {
  const [dark, setDark] = useState(true);

  function toggleTheme() {
    const root = document.documentElement;
    root.classList.toggle("dark");
    setDark(root.classList.contains("dark"));
  }

  return (
    <Providers>
      <AppShell
        titlebar={<Titlebar dark={dark} onToggleTheme={toggleTheme} />}
        sidebar={<Sidebar />}
      >
        <Dashboard />
      </AppShell>
      <Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
    </Providers>
  );
}
