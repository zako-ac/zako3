"use client"

import { useEffect, useState } from "react"
import { useTheme } from "next-themes"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Globe, Moon, Sun } from "lucide-react"

type SettingsMenuProps = {
  lang: "ko" | "en"
  onLanguageChange: (lang: "ko" | "en") => void
}

const labels = {
  ko: { language: "언어", korean: "한국어", english: "English" },
  en: { language: "Language", korean: "한국어", english: "English" },
}

export function SettingsMenu({ lang, onLanguageChange }: SettingsMenuProps) {
  const { theme, setTheme } = useTheme()
  const [mounted, setMounted] = useState(false)
  const t = labels[lang]

  useEffect(() => setMounted(true), [])

  return (
    <div className="flex items-center gap-2">
      <Button variant="ghost" size="icon" className="rounded-full" onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
        {mounted && (theme === "dark" ? <Sun className="h-5 w-5" /> : <Moon className="h-5 w-5" />)}
      </Button>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" className="rounded-full">
            <Globe className="h-5 w-5" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-48">
          <DropdownMenuLabel>{t.language}</DropdownMenuLabel>
          <DropdownMenuItem onClick={() => onLanguageChange("ko")} className={lang === "ko" ? "bg-accent" : ""}>
            {t.korean}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => onLanguageChange("en")} className={lang === "en" ? "bg-accent" : ""}>
            {t.english}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}
