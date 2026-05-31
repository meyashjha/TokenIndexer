export function FeedSkeleton() {
  return (
    <div className="space-y-4" aria-busy="true" aria-label="Loading feed items">
      {Array.from({ length: 6 }).map((_, index) => (
        <div key={index} className="h-16 rounded-2xl shimmer-bg border border-white/5" />
      ))}
    </div>
  );
}

export function BuyersSkeleton() {
  return (
    <div className="space-y-3" aria-busy="true" aria-label="Loading token transactions">
      <div className="h-10 rounded-xl shimmer-bg border border-white/5" />
      {Array.from({ length: 5 }).map((_, index) => (
        <div key={index} className="h-14 rounded-xl shimmer-bg border border-white/5" />
      ))}
    </div>
  );
}
