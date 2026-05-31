import { AlertCircle, Database } from "lucide-react";

interface FeedErrorStateProps {
  message: string | null;
  onRetry: () => void;
}

export function FeedErrorState({ message, onRetry }: FeedErrorStateProps) {
  return (
    <div className="glass-panel p-6 rounded-2xl border-red-500/20 text-sm space-y-4 text-center">
      <div className="flex items-center justify-center gap-2 font-bold text-red-400">
        <AlertCircle className="h-5 w-5" />
        Could not connect to indexer API
      </div>
      <p className="text-zinc-400 max-w-md mx-auto">
        {message ?? "Please verify your backend service is running locally on port 8080 or config is correct."}
      </p>
      <button
        type="button"
        onClick={onRetry}
        className="inline-flex items-center justify-center rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-300 border border-red-500/20 px-5 py-2.5 text-xs font-semibold transition"
      >
        Retry connection
      </button>
    </div>
  );
}

interface FeedEmptyStateProps {
  onRetry: () => void;
}

export function FeedEmptyState({ onRetry }: FeedEmptyStateProps) {
  return (
    <div className="glass-panel p-10 rounded-2xl text-center space-y-4">
      <Database className="h-8 w-8 text-purple-400 mx-auto animate-pulse" />
      <div className="max-w-md mx-auto space-y-2">
        <p className="text-sm font-bold text-white">No indexed tokens discovered yet</p>
        <p className="text-xs text-zinc-400 leading-relaxed">
          Once the Scout node discovers token launches on Solana chain and logs them to PostgreSQL, they will populate here instantly.
        </p>
      </div>
      <button
        type="button"
        onClick={onRetry}
        className="inline-flex items-center justify-center rounded-xl bg-purple-600 hover:bg-purple-500 px-5 py-2 text-xs font-semibold text-white transition shadow-[0_0_15px_rgba(168,85,247,0.3)]"
      >
        Check again
      </button>
    </div>
  );
}
