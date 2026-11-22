import { invoke } from "@tauri-apps/api/core";

export type Product = {
  id: string;
  name: string;
  size: string | null;
  notes: string | null;
  version: number;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  stock: number;
};
export type Receipt = {
  id: string;
  containerNumber: string;
  receivedDate: number;
  status: "draft" | "finalized";
};
export type Item = {
  id: string;
  productName: string;
  quantityReceived: number;
};
export type Totals = { totalSkus: number; totalUnits: number };
export type Flag = {
  id: string;
  productName: string;
  transactionQuantity: number;
  transactionTimestamp: number;
  stockNow: number;
};
export type PreviewRow = {
  productId: string;
  productName: string;
  opening: number;
  totalIn: number;
  totalOut: number;
  expected: number;
};

type Kind = "ok" | "warn" | "error";
let listener: (msg: string, kind: Kind) => void = () => {};
export const onMessage = (f: typeof listener) => {
  listener = f;
};
export const notify = (msg: string, kind: Kind = "ok") => listener(msg, kind);

// Calls a Rust command. On failure it shows the error banner and returns undefined.
export async function call<T = null>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T | undefined> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    notify(String(e), "error");
    return undefined;
  }
}

export const today = () => new Date().toLocaleDateString("en-CA"); // YYYY-MM-DD, local time
export const fmtDate = (ms: number) => new Date(ms).toLocaleDateString();
export const wholeNumber = (v?: string) =>
  v !== undefined && v.trim() !== "" && Number.isInteger(Number(v));
