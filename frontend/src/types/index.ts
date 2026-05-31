export type Token = {
  mint_address: string;
  creation_timestamp: string;
  launchpad_source: "PumpFun" | "Raydium" | "Unknown" | string;
  slot_number: number;
  analyzed: boolean;
  last_indexed_signature?: string | null;
  last_indexed_at?: string | null;
};

export type BuyerTransaction = {
  signature: string;
  token_mint: string;
  buyer_address: string;
  amount: number;
  slot_number: number;
  timestamp: string;
};

export type PaginatedResponse<T> = {
  data: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
};
