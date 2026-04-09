import Image from "next/image"
import { Button } from "@/components/ui/button"
import zakopsa from "../../public/assets/zakopsa.png"

function GithubIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor">
      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.3 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61-.546-1.385-1.335-1.755-1.335-1.755-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 21.795 24 17.295 24 12c0-6.63-5.37-12-12-12" />
    </svg>
  )
}

export function Footer() {
  return (
    <footer className="border-t border-border/50 bg-secondary/20">
      <div className="max-w-7xl mx-auto px-4 py-12 md:py-16">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8 md:gap-12">
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <Image
                className="w-8 h-8 rounded-lg"
                src={zakopsa}
                alt="Zakopsa"
                width={32}
                height={32}
              />
              <span className="text-xl font-bold">자코</span>
            </div>
            <p className="text-sm text-muted-foreground leading-relaxed">통화 채널을 더 다채롭게 만드는 디스코드 봇</p>
            <div className="flex gap-2">
              <Button asChild variant="ghost" size="icon" className="rounded-full">
                <a href="https://github.com/zako-ac" target="_blank" rel="noopener noreferrer">
                  <GithubIcon className="h-4 w-4" />
                </a>
              </Button>
            </div>
          </div>

          <div className="space-y-4">
            <h3 className="font-semibold">리소스</h3>
            <ul className="space-y-2 text-sm text-muted-foreground">
              <li>
                <a href={`${process.env.NEXT_PUBLIC_DOCS_URL}/manual`} className="hover:text-foreground transition-colors">
                  문서
                </a>
              </li>
              <li>
                <a href={process.env.NEXT_PUBLIC_DOCS_URL} className="hover:text-foreground transition-colors">
                  가이드
                </a>
              </li>
              <li>
                <a href={`${process.env.NEXT_PUBLIC_DOCS_URL}/api`} className="hover:text-foreground transition-colors">
                  API
                </a>
              </li>
              <li>
                <a href="#" className="hover:text-foreground transition-colors">
                  지원
                </a>
              </li>
            </ul>
          </div>

          <div className="space-y-4">
            <h3 className="font-semibold">ZAKO</h3>
            <ul className="space-y-2 text-sm text-muted-foreground">
              <li>
                <a href="#" className="hover:text-foreground transition-colors">
                  블로그
                </a>
              </li>
              <li>
                <a href="#" className="hover:text-foreground transition-colors">
                  개인정보처리방침
                </a>
              </li>
              <li>
                <a href="#" className="hover:text-foreground transition-colors">
                  이용약관
                </a>
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-12 pt-8 border-t border-border/50 flex flex-col md:flex-row items-center justify-between gap-4">
          <p className="text-sm text-muted-foreground">© 2025 Walrus Lab. All rights reserved.</p>
          <p className="text-sm text-muted-foreground">Made with ❤️ for Discord communities</p>
        </div>
      </div>
    </footer>
  )
}
