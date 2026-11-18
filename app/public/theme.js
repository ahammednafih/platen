try {
  const theme = localStorage.getItem("theme");
  const accent = localStorage.getItem("accent");
  if (theme === "light" || theme === "dark") {
    document.querySelector('meta[name="color-scheme"]').content = theme;
    document.documentElement.dataset.theme = theme;
  }
  if (accent && accent !== "ink") document.documentElement.dataset.accent = accent;
} catch {}
