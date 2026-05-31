import { AlertCircle, ArrowLeft, Database, ExternalLink, TrendingUp } from "lucide-react";
import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { CopyButton } from "../components/CopyButton";
import { BuyersSkeleton } from "../components/Skeletons";
import { Token, BuyerTransaction, PaginatedResponse } from "../types";

const API_URL = import.meta.env.VITE_API_URL ?? "/api";

export function TokenBuyers() {
  const { mint } = useParams<{ mint: string }>();
  const navigate = useNavigate();

  const [tokenInfo, setTokenInfo] = useState<Token | null>(null);
  const [buyers, setBuyers] = useState<BuyerTransaction[]>([]);
  const [buyersStatus, setBuyersStatus] = useState<"loading" | "success" | "empty" | "error">("loading");
  const [buyersError, setBuyersError] = useState<string | null>(null);

  useEffect(() => {
    if (!mint) return;

    let active = true;

    // 1. Fetch token details
    const fetchTokenInfo = async () => {
      try {
        const res = await fetch(`${API_URL}/tokens/${mint}`);
        if (res.ok && active) {
          const data = (await res.json()) as Token;
          setTokenInfo(data);
        }
      } catch (err) {
        console.error("Failed to load token metadata", err);
      }
    };

    // 2. Fetch buyers list
    const loadBuyers = async () => {
      setBuyersStatus("loading");
      setBuyersError(null);
      try {
        const response = await fetch(`${API_URL}/tokens/${mint}/transactions?page_size=50`);
        if (!response.ok) {
          throw new Error(`Backend returned ${response.status}`);
        }
        const payload = (await response.json()) as PaginatedResponse<BuyerTransaction>;
        if (active) {
          setBuyers(payload.data);
          setBuyersStatus(payload.data.length ? "success" : "empty");
        }
      } catch (err) {
        if (active) {
          setBuyersError(err instanceof Error ? err.message : "Could not fetch buyers");
          setBuyersStatus("error");
        }
      }
    };

    void fetchTokenInfo();
    void loadBuyers();

    return () => {
      active = false;
    };
  }, [mint]);

  if (!mint) {
    return (
      <div className="glass-panel p-6 rounded-2xl border-red-500/20 text-center space-y-4">
        <AlertCircle className="h-8 w-8 text-red-500 mx-auto" />
        <p className="text-zinc-200">No token mint address specified in the route.</p>
        <button
          onClick={() => navigate("/")}
          className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold bg-white/5 hover:bg-white/10 text-zinc-300 transition"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          Go Home
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Detail Token Header Card */}
      <div className="glass-panel p-6 rounded-2xl space-y-4">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <span className="text-xs font-bold uppercase tracking-wider text-purple-400 font-mono">
                TOKEN CONTRACT
              </span>
              {tokenInfo && (
                <span
                  className={`px-2 py-0.5 rounded-md text-[10px] font-bold border ${
                    tokenInfo.launchpad_source === "PumpFun"
                      ? "bg-purple-500/10 text-purple-300 border-purple-500/20"
                      : tokenInfo.launchpad_source === "Raydium"
                      ? "bg-cyan-500/10 text-cyan-300 border-cyan-500/20"
                      : "bg-zinc-500/10 text-zinc-300 border-zinc-500/20"
                  }`}
                >
                  {tokenInfo.launchpad_source === "PumpFun" ? "Pump.fun" : tokenInfo.launchpad_source}
                </span>
              )}
            </div>
            <h2 className="text-xl sm:text-2xl font-bold tracking-tight text-white flex items-center font-mono break-all">
              {mint}
              <CopyButton text={mint} />
            </h2>
          </div>

          <div className="flex items-center gap-3">
            <a
              href={`https://solscan.io/token/${mint}`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-semibold bg-cyan-600/20 hover:bg-cyan-600/30 text-cyan-300 border border-cyan-500/20 transition shadow-[0_0_15px_rgba(6,182,212,0.1)]"
            >
              View on Solscan
              <ExternalLink className="h-3.5 w-3.5" />
            </a>
          </div>
        </div>

        {/* Metadata Info Badges Grid */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 pt-4 border-t border-white/5 text-xs font-mono">
          <div>
            <span className="block text-zinc-500 mb-1">CREATION SLOT</span>
            <span className="text-zinc-200 font-semibold text-sm">
              {tokenInfo ? tokenInfo.slot_number.toLocaleString() : "..."}
            </span>
          </div>
          <div>
            <span className="block text-zinc-500 mb-1">DISCOVERED AT</span>
            <span className="text-zinc-200 font-semibold text-sm">
              {tokenInfo ? formatDate(tokenInfo.creation_timestamp) : "..."}
            </span>
          </div>
          <div>
            <span className="block text-zinc-500 mb-1">STATUS</span>
            <span className="text-zinc-200 font-semibold text-sm flex items-center gap-1.5">
              {tokenInfo ? (
                tokenInfo.last_indexed_at ? (
                  <>
                    <span className="w-2 h-2 rounded-full bg-green-500" />
                    Fully Indexed
                  </>
                ) : (
                  <>
                    <span className="w-2 h-2 rounded-full bg-rose-500 animate-pulse" />
                    Pending Indexing
                  </>
                )
              ) : (
                "..."
              )}
            </span>
          </div>
          <div>
            <span className="block text-zinc-500 mb-1">LAST INDEXED</span>
            <span className="text-zinc-200 font-semibold text-sm">
              {tokenInfo && tokenInfo.last_indexed_at
                ? formatDate(tokenInfo.last_indexed_at)
                : "Pending"}
            </span>
          </div>
        </div>
      </div>

      {/* Buyers Headline */}
      <div className="flex items-center justify-between pt-2">
        <h3 className="text-lg font-bold text-white flex items-center gap-2">
          <TrendingUp className="h-4 w-4 text-purple-400" />
          Latest Buyers & Transactions
        </h3>
        <span className="text-xs font-mono text-zinc-400">
          {buyers.length} transactions indexed
        </span>
      </div>

      {/* Buyers Data Views */}
      {buyersStatus === "loading" && <BuyersSkeleton />}
      {buyersStatus === "error" && (
        <div className="glass-panel p-6 rounded-xl border-red-500/20 text-center space-y-3">
          <AlertCircle className="h-8 w-8 text-red-500 mx-auto" />
          <p className="text-red-400 text-sm font-semibold">{buyersError}</p>
        </div>
      )}
      {buyersStatus === "empty" && (
        <div className="glass-panel p-8 rounded-xl text-center space-y-4">
          <div className="w-12 h-12 rounded-full bg-purple-500/10 border border-purple-500/20 flex items-center justify-center mx-auto text-purple-400">
            <Database className="h-6 w-6 animate-pulse" />
          </div>
          <div className="max-w-md mx-auto space-y-2">
            <p className="text-sm font-bold text-zinc-200">No buyer transactions found</p>
            <p className="text-xs text-zinc-400 leading-relaxed">
              This token is newly discovered. Our indexing engine is scanning transaction histories. Transactions will populate shortly as indexing concludes.
            </p>
          </div>
        </div>
      )}
      {buyersStatus === "success" && (
        <div className="glass-panel overflow-hidden rounded-2xl">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[700px] border-collapse text-left text-xs font-mono">
              <thead className="border-b border-white/5 bg-white/2 text-zinc-400 uppercase tracking-wider">
                <tr>
                  <th className="px-5 py-3.5">Wallet Address</th>
                  <th className="px-5 py-3.5 text-right">Amount Bought</th>
                  <th className="px-5 py-3.5">Block Slot</th>
                  <th className="px-5 py-3.5">Transaction Time</th>
                  <th className="px-5 py-3.5 text-right">Solscan Link</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {buyers.map((tx) => (
                  <tr key={tx.signature} className="hover:bg-white/2 transition">
                    <td className="px-5 py-4 text-zinc-300">
                      <div className="flex items-center gap-1.5">
                        <span className="truncate max-w-[200px]" title={tx.buyer_address}>
                          {tx.buyer_address}
                        </span>
                        <CopyButton text={tx.buyer_address} />
                      </div>
                    </td>
                    <td className="px-5 py-4 text-right font-bold text-cyan-400">
                      {formatAmount(tx.amount)}
                    </td>
                    <td className="px-5 py-4 text-zinc-400">{tx.slot_number.toLocaleString()}</td>
                    <td className="px-5 py-4 text-zinc-400">{formatDate(tx.timestamp)}</td>
                    <td className="px-5 py-4 text-right">
                      <a
                        href={`https://solscan.io/tx/${tx.signature}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="inline-flex items-center gap-1 px-2.5 py-1 rounded bg-purple-500/10 hover:bg-purple-500/20 text-purple-300 border border-purple-500/20 transition text-[11px]"
                      >
                        Explore
                        <ExternalLink className="h-3 w-3" />
                      </a>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
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

function formatAmount(amount: number) {
  if (amount === 0) return "0.00";
  if (amount >= 1_000_000) {
    return `${(amount / 1_000_000).toFixed(2)}M`;
  }
  if (amount >= 1_000) {
    return `${(amount / 1_000).toFixed(2)}K`;
  }
  return amount.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
