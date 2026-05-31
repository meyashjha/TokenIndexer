import { Compass, ArrowLeft } from "lucide-react";
import { Routes, Route, Link, useLocation } from "react-router-dom";
import { TokenFeed } from "./pages/TokenFeed";
import { TokenBuyers } from "./pages/TokenBuyers";

export function App() {
  const location = useLocation();
  const isDetailPage = location.pathname.startsWith("/tokens/");

  return (
    <main className="min-h-screen text-zinc-100 flex flex-col font-sans">
      {/* Glow Ambient Spots */}
      <div className="absolute top-0 left-1/4 w-96 h-96 bg-purple-600/10 rounded-full blur-[120px] pointer-events-none" />
      <div className="absolute top-10 right-1/4 w-96 h-96 bg-cyan-600/10 rounded-full blur-[120px] pointer-events-none" />

      {/* Top Header */}
      <header className="border-b border-white/5 bg-[#0a0b10]/40 backdrop-blur-md sticky top-0 z-50">
        <div className="mx-auto max-w-7xl px-4 py-4 sm:px-6 lg:px-8 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Link to="/" className="flex items-center gap-2 group">
              <div className="relative">
                <Compass className="h-6 w-6 text-purple-400 group-hover:rotate-45 transition-transform duration-300" />
                <div className="absolute -inset-0.5 bg-purple-500 rounded-full blur opacity-30 group-hover:opacity-60 transition" />
              </div>
              <span className="text-xl font-bold tracking-tight bg-gradient-to-r from-white via-purple-300 to-cyan-300 bg-clip-text text-transparent">
                TokenIndexer
              </span>
            </Link>
            <div className="hidden sm:flex items-center gap-2 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-cyan-500/10 text-cyan-300 border border-cyan-500/20">
              <span className="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-ping" />
              LIVE METRICS
            </div>
          </div>
          {isDetailPage && (
            <Link
              to="/"
              className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium bg-white/5 hover:bg-white/10 text-zinc-300 transition border border-white/5"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              Back to List
            </Link>
          )}
        </div>
      </header>

      {/* Main Content Router */}
      <div className="flex-1 mx-auto w-full max-w-7xl px-4 py-8 sm:px-6 lg:px-8 z-10">
        <Routes>
          <Route path="/" element={<TokenFeed />} />
          <Route path="/tokens/:mint" element={<TokenBuyers />} />
        </Routes>
      </div>

      {/* Footer */}
      <footer className="border-t border-white/5 bg-zinc-950/40 py-6 text-center text-xs text-zinc-500 font-mono mt-auto">
        <p>© 2026 WhaleTracker Protocol // Solana Intel Indexer v0.1.0</p>
      </footer>
    </main>
  );
}
