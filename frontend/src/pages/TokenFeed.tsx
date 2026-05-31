import { Database, Search, RefreshCw, ArrowLeft } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { CopyButton } from "../components/CopyButton";
import { FeedSkeleton } from "../components/Skeletons";
import { FeedErrorState, FeedEmptyState } from "../components/StateViews";
import { Token, PaginatedResponse } from "../types";

const API_URL = import.meta.env.VITE_API_URL ?? "/api";

export function TokenFeed() {
  const [tokens, setTokens] = useState<Token[]>([]);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<"loading" | "success" | "empty" | "error">("loading");
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  const loadTokens = async () => {
    setStatus("loading");
    setError(null);

    try {
      const response = await fetch(`${API_URL}/tokens?page_size=30`);
      if (!response.ok) {
        throw new Error(`Backend returned ${response.status}`);
      }
      const payload = (await response.json()) as PaginatedResponse<Token>;
      setTokens(payload.data);
      setStatus(payload.data.length ? "success" : "empty");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reach the backend");
      setStatus("error");
    }
  };

  useEffect(() => {
    void loadTokens();
  }, []);

  const filteredTokens = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return tokens;
    }
    return tokens.filter((token) => token.mint_address.toLowerCase().includes(normalized));
  }, [query, tokens]);

  return (
    <div className="space-y-6">
      {/* Welcome Banner */}
      <div className="glass-panel p-6 rounded-2xl relative overflow-hidden">
        <div className="absolute top-0 right-0 w-64 h-64 bg-gradient-to-br from-purple-500/5 to-cyan-500/5 rounded-full blur-3xl pointer-events-none" />
        <div className="max-w-3xl space-y-2">
          <h2 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-white">
            Early Buyer & Whale Intelligence
          </h2>
          <p className="text-zinc-400 text-sm leading-relaxed">
            Real-time indexer monitoring newly created tokens on Solana launchpads. Detect whale buys early, watch wallet movements, and study early buyer behaviors before they hit the mainstream.
          </p>
        </div>
      </div>

      {/* Filter & Metric Header */}
      <div className="flex flex-col sm:flex-row gap-4 justify-between items-stretch sm:items-center">
        <div className="flex items-center gap-2 text-xs font-mono text-zinc-400">
          <Database className="h-4 w-4 text-cyan-400" />
          <span>{tokens.length} tokens detected</span>
        </div>

        <div className="flex items-center gap-3">
          {/* Search Bar */}
          <div className="relative flex-1 sm:w-80">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-500" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              type="search"
              placeholder="Search by token mint address..."
              className="w-full rounded-xl bg-zinc-950/80 border border-white/10 py-2.5 pl-10 pr-4 text-sm text-zinc-100 placeholder:text-zinc-500 focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/30 transition-all font-mono"
            />
          </div>

          {/* Refresh Button */}
          <button
            type="button"
            onClick={() => void loadTokens()}
            disabled={status === "loading"}
            className="inline-flex items-center justify-center p-2.5 rounded-xl bg-zinc-950/80 border border-white/10 hover:bg-white/5 text-zinc-400 hover:text-zinc-200 transition disabled:opacity-50"
            title="Refresh Feed"
          >
            <RefreshCw className={`h-4 w-4 ${status === "loading" ? "animate-spin" : ""}`} />
          </button>
        </div>
      </div>

      {/* Data Feed */}
      {status === "loading" && <FeedSkeleton />}
      {status === "error" && <FeedErrorState message={error} onRetry={loadTokens} />}
      {status === "empty" && <FeedEmptyState onRetry={loadTokens} />}
      {status === "success" && (
        <TokenListTable
          tokens={filteredTokens}
          onSelectToken={(mint) => navigate(`/tokens/${mint}`)}
        />
      )}
    </div>
  );
}

interface TokenListTableProps {
  tokens: Token[];
  onSelectToken: (mint: string) => void;
}

function TokenListTable({ tokens, onSelectToken }: TokenListTableProps) {
  if (tokens.length === 0) {
    return (
      <div className="glass-panel rounded-2xl p-8 text-center text-sm text-zinc-400">
        No tokens match that mint address. Try checking again or search for another query.
      </div>
    );
  }

  return (
    <div className="glass-panel overflow-hidden rounded-2xl fade-in">
      <div className="overflow-x-auto">
        <table className="w-full min-w-[760px] border-collapse text-left text-sm">
          <thead className="border-b border-white/5 bg-white/2 text-xs font-mono uppercase tracking-wider text-zinc-400">
            <tr>
              <th className="px-5 py-3.5">Mint</th>
              <th className="px-5 py-3.5">Source</th>
              <th className="px-5 py-3.5">Slot</th>
              <th className="px-5 py-3.5">Created</th>
              <th className="px-5 py-3.5">Indexed status</th>
              <th className="px-5 py-3.5 text-right">Intel Details</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-white/5 font-mono text-xs">
            {tokens.map((token) => (
              <tr
                key={token.mint_address}
                onClick={() => onSelectToken(token.mint_address)}
                className="hover:bg-white/[0.03] transition-colors cursor-pointer group"
              >
                <td className="px-5 py-4 max-w-[280px]">
                  <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                    <span className="block truncate text-zinc-200 group-hover:text-purple-300 font-semibold transition-colors">
                      {token.mint_address}
                    </span>
                    <CopyButton text={token.mint_address} />
                  </div>
                </td>
                <td className="px-5 py-4">
                  <span
                    className={`inline-block px-2.5 py-0.5 rounded-full text-[10px] font-bold border ${token.launchpad_source === "PumpFun"
                        ? "bg-purple-500/10 text-purple-300 border-purple-500/20"
                        : token.launchpad_source === "Raydium"
                          ? "bg-cyan-500/10 text-cyan-300 border-cyan-500/20"
                          : "bg-zinc-500/10 text-zinc-400 border-zinc-500/20"
                      }`}
                  >
                    {token.launchpad_source === "PumpFun" ? "Pump.fun" : token.launchpad_source}
                  </span>
                </td>
                <td className="px-5 py-4 text-zinc-400">{token.slot_number.toLocaleString()}</td>
                <td className="px-5 py-4 text-zinc-400">{formatDate(token.creation_timestamp)}</td>
                <td className="px-5 py-4">
                  {token.last_indexed_at ? (
                    <span className="inline-flex items-center gap-1 text-green-400 text-[11px] font-medium bg-green-500/10 border border-green-500/20 px-2 py-0.5 rounded">
                      <span className="w-1.5 h-1.5 rounded-full bg-green-400" />
                      {formatDate(token.last_indexed_at)}
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] font-medium animate-pulse-glow border">
                      <span className="w-1.5 h-1.5 rounded-full bg-rose-400" />
                      Pending
                    </span>
                  )}
                </td>
                <td className="px-5 py-4 text-right">
                  <span className="inline-flex items-center gap-1 text-xs font-semibold text-purple-400 group-hover:text-purple-300 transition-colors">
                    Analyze Buyers
                    <ArrowLeft className="h-3 w-3 rotate-180" />
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function formatDate(value: string) {
  if (!value) return "N/A";
  try {
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }).format(new Date(value));
  } catch {
    return value;
  }
}
