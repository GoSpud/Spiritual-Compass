import { createContext, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

export type ThemeName = "light" | "dark" | "reading";
export type TextColorName = "auto" | "white" | "black" | "yellow" | "green" | "blue" | "gray";

const TEXT_COLOR_HEX: Record<Exclude<TextColorName, "auto">, string> = {
  white: "#ffffff",
  black: "#000000",
  yellow: "#ffff00",
  green: "#00ff00",
  blue: "#0000ff",
  gray: "#808080",
};

export const TEXT_COLOR_ORDER: TextColorName[] = ["auto", "black", "white", "yellow", "green", "blue", "gray"];

interface ThemeContextValue {
  theme: ThemeName;
  setTheme: (t: ThemeName) => void;
  textColor: TextColorName;
  setTextColor: (c: TextColorName) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: "light",
  setTheme: () => {},
  textColor: "auto",
  setTextColor: () => {},
});

export function useTheme() {
  return useContext(ThemeContext);
}

function applyTheme(theme: ThemeName) {
  const root = document.documentElement;
  root.dataset.theme = theme;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeName>("light");
  const [textColor, setTextColorState] = useState<TextColorName>("auto");

  useEffect(() => {
    invoke<string>("get_setting", { key: "theme" })
      .then((stored) => {
        if (stored === "dark" || stored === "reading" || stored === "light") {
          setThemeState(stored);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    invoke<string>("get_setting", { key: "textColor" })
      .then((stored) => {
        if (stored && TEXT_COLOR_ORDER.includes(stored as TextColorName)) {
          setTextColorState(stored as TextColorName);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    applyTheme(theme);
    const root = document.documentElement;
    // "auto" inherits the active theme's own text color.
    root.style.setProperty(
      "--text-color",
      textColor === "auto" ? "var(--text)" : TEXT_COLOR_HEX[textColor],
    );
  }, [theme, textColor]);

  const setTheme = (t: ThemeName) => {
    setThemeState(t);
    invoke("set_setting", { key: "theme", value: t }).catch(() => {});
  };

  const setTextColor = (c: TextColorName) => {
    setTextColorState(c);
    invoke("set_setting", { key: "textColor", value: c }).catch(() => {});
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme, textColor, setTextColor }}>
      {children}
    </ThemeContext.Provider>
  );
}
