import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Product = { id: string; name: string };
type Receipt = {
    id: string;
    containerNumber: string;
    receivedDate: number;
    status: "draft" | "finalized";
};
type Item = { id: string; productId: string; quantityReceived: number };
type Totals = { totalSkus: number; totalUnits: number };

export default function Receipts({ products }: { products: Product[] }) {
    const [receipts, setReceipts] = useState<Receipt[]>([]);
    const [selected, setSelected] = useState<Receipt | null>(null);
    const [items, setItems] = useState<Item[]>([]);
    const [totals, setTotals] = useState<Totals | null>(null);
    const [containerNumber, setContainerNumber] = useState("");
    const [productId, setProductId] = useState("");
    const [qty, setQty] = useState(1);

    async function loadReceipts() {
        setReceipts(await invoke<Receipt[]>("list_receipts"));
    }

    async function open(r: Receipt) {
        setSelected(r);
        setItems(await invoke<Item[]>("list_items", { receiptId: r.id }));
        setTotals(await invoke<Totals>("receipt_totals", { receiptId: r.id }));
    }

    async function createReceipt() {
        if (!containerNumber.trim()) return;
        await invoke("create_receipt", {
            containerNumber,
            receivedDate: Date.now(),
            notes: null,
        });
        setContainerNumber("");
        loadReceipts();
    }

    async function addItem() {
        if (!selected || !productId || qty < 1) return;
        try {
            await invoke("add_item", {
                receiptId: selected.id,
                productId,
                quantity: qty,
                notes: null,
            });
            open(selected);
        } catch (e) {
            alert(`Could not add item: ${e}`);
        }
    }

    async function finalize() {
        if (!selected) return;
        try {
            await invoke("finalize_receipt", { receiptId: selected.id });
            await loadReceipts();
            open({ ...selected, status: "finalized" });
        } catch (e) {
            alert(`Could not finalize: ${e}`);
        }
    }

    useEffect(() => {
        loadReceipts();
    }, []);

    const nameOf = (id: string) =>
        products.find((p) => p.id === id)?.name ?? "Unknown";

    return (
        <div>
            <h2>Receipts</h2>
            <input
                value={containerNumber}
                onChange={(e) => setContainerNumber(e.target.value)}
                placeholder="Container number, e.g. BALE-001"
            />
            <button onClick={createReceipt}>New receipt</button>

            <ul>
                {receipts.map((r) => (
                    <li key={r.id}>
                        <button onClick={() => open(r)}>
                            {r.containerNumber} ({r.status})
                        </button>
                    </li>
                ))}
            </ul>

            {selected && (
                <section>
                    <h3>{selected.containerNumber}</h3>
                    <ul>
                        {items.map((i) => (
                            <li key={i.id}>
                                {nameOf(i.productId)} × {i.quantityReceived}
                            </li>
                        ))}
                    </ul>
                    {totals && (
                        <p>
                            {totals.totalSkus} products, {totals.totalUnits} units
                        </p>
                    )}

                    {selected.status === "draft" && (
                        <>
                            <select
                                value={productId}
                                onChange={(e) => setProductId(e.target.value)}
                            >
                                <option value="">Choose product</option>
                                {products.map((p) => (
                                    <option key={p.id} value={p.id}>
                                        {p.name}
                                    </option>
                                ))}
                            </select>
                            <input
                                type="number"
                                min={1}
                                value={qty}
                                onChange={(e) => setQty(Number(e.target.value))}
                            />
                            <button onClick={addItem}>Add item</button>
                            <button onClick={finalize}>Finalize</button>
                        </>
                    )}
                </section>
            )}
        </div>
    );
}
