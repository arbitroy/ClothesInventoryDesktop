import { useEffect, useState } from "react";
import {
    call,
    fmtDate,
    notify,
    wholeNumber,
    type Item,
    type Product,
    type Receipt,
    type Totals,
} from "./api";

export default function Receipts() {
    const [receipts, setReceipts] = useState<Receipt[]>([]);
    const [products, setProducts] = useState<Product[]>([]);
    const [sel, setSel] = useState<Receipt | null>(null);
    const [items, setItems] = useState<Item[]>([]);
    const [totals, setTotals] = useState<Totals | null>(null);
    const [number, setNumber] = useState("");
    const [productId, setProductId] = useState("");
    const [qty, setQty] = useState("1");

    const loadList = async () =>
        setReceipts((await call<Receipt[]>("list_receipts")) ?? []);
    useEffect(() => {
        loadList();
        call<Product[]>("list_products_with_stock").then((p) =>
            setProducts(p ?? []),
        );
    }, []);

    async function open(r: Receipt) {
        setSel(r);
        setItems((await call<Item[]>("list_items", { receiptId: r.id })) ?? []);
        setTotals(
            (await call<Totals>("receipt_totals", { receiptId: r.id })) ?? null,
        );
    }

    async function create() {
        if (!number.trim()) return;
        const id = await call<string>("create_receipt", {
            containerNumber: number.trim(),
            receivedDate: Date.now(),
            notes: null,
        });
        if (id === undefined) return;
        setNumber("");
        loadList();
    }

    async function addItem() {
        if (!sel || !productId || !wholeNumber(qty)) return;
        const id = await call<string>("add_item", {
            receiptId: sel.id,
            productId,
            quantity: Number(qty),
            notes: null,
        });
        if (id !== undefined) open(sel);
    }

    async function finalize() {
        if (
            !sel ||
            !confirm(
                "Finalize this receipt? Stock goes up and the receipt can't be edited.",
            )
        )
            return;
        if (
            (await call<number>("finalize_receipt", { receiptId: sel.id })) ===
            undefined
        )
            return;
        notify("Receipt finalized. Stock updated.");
        await loadList();
        open({ ...sel, status: "finalized" });
    }

    return (
        <section>
            <h2>Container receipts</h2>
            <div className="split">
                <div>
                    <div className="row">
                        <input
                            value={number}
                            onChange={(e) => setNumber(e.target.value)}
                            placeholder="Container no., e.g. BALE-001"
                        />
                        <button onClick={create}>New receipt</button>
                    </div>
                    <div className="list">
                        {receipts.map((r) => (
                            <button
                                key={r.id}
                                className={sel?.id === r.id ? "on" : ""}
                                onClick={() => open(r)}
                            >
                                <strong>{r.containerNumber}</strong>
                                <span>
                                    {fmtDate(r.receivedDate)} · {r.status}
                                </span>
                            </button>
                        ))}
                        {!receipts.length && <p className="empty">No receipts yet.</p>}
                    </div>
                </div>

                <div>
                    {!sel && <p className="empty">Pick a receipt to see its items.</p>}
                    {sel && (
                        <>
                            <h3>
                                {sel.containerNumber}{" "}
                                <span className={"pill " + sel.status}>{sel.status}</span>
                            </h3>
                            <table>
                                <thead>
                                    <tr>
                                        <th>Product</th>
                                        <th className="num">Quantity</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {items.map((i) => (
                                        <tr key={i.id}>
                                            <td>{i.productName}</td>
                                            <td className="num">{i.quantityReceived}</td>
                                        </tr>
                                    ))}
                                    {!items.length && (
                                        <tr>
                                            <td colSpan={2} className="empty">
                                                No items yet.
                                            </td>
                                        </tr>
                                    )}
                                </tbody>
                            </table>
                            {totals && (
                                <p className="hint">
                                    {totals.totalSkus} products, {totals.totalUnits} units
                                </p>
                            )}
                            {sel.status === "draft" && (
                                <div className="row">
                                    <select
                                        value={productId}
                                        onChange={(e) => setProductId(e.target.value)}
                                    >
                                        <option value="">Choose product</option>
                                        {products.map((p) => (
                                            <option key={p.id} value={p.id}>
                                                {p.name}
                                                {p.size ? ` (${p.size})` : ""}
                                            </option>
                                        ))}
                                    </select>
                                    <input
                                        className="short"
                                        type="number"
                                        min={1}
                                        value={qty}
                                        onChange={(e) => setQty(e.target.value)}
                                    />
                                    <button onClick={addItem}>Add item</button>
                                    <button className="ghost" onClick={finalize}>
                                        Finalize receipt
                                    </button>
                                </div>
                            )}
                        </>
                    )}
                </div>
            </div>
        </section>
    );
}
